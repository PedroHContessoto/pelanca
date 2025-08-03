// Motor de Xadrez - Interface UCI para Arena Chess
use pelanca::*;
use pelanca::engine::PerftTT;
use pelanca::search::*;
use std::io;
use std::time::{Duration, Instant};
use rayon::prelude::*;

fn main() {
    // Inicializa as dependências do motor
    crate::moves::magic_bitboards::init_magic_bitboards();
    
    // PERFORMANCE CRÍTICA: Reporta suporte de intrinsics (popcount, BMI, etc.)
    #[cfg(debug_assertions)]
    pelanca::utils::init_intrinsics();
    
    let mut board = Board::new();
    let mut tt = PerftTT::new(); // Usando PerftTT como transposition table temporária
    let mut use_opening_book = true;
    let mut moves_played = 0u16;
    let mut threads = num_cpus::get().max(1);

    // Loop principal que espera por comandos da GUI
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let commands: Vec<&str> = input.trim().split_whitespace().collect();

        if let Some(&command) = commands.get(0) {
            match command {
                "uci" => {
                    println!("id name Pelanca 1.0");
                    println!("id author Pedro Contessoto");
                    
                    println!("uciok");
                }
                "isready" => {
                    println!("readyok");
                }
                "position" => {
                    moves_played = handle_position_command(&mut board, &commands, &mut tt);
                }
                "go" => {
                    handle_go_command(&board, &mut tt, use_opening_book, &commands, moves_played, threads);
                }
                "setoption" => {
                    handle_setoption_command(&commands, &mut use_opening_book, &mut threads);
                }
                "stop" => {
                    println!("bestmove (none)");
                }
                "quit" => {
                    break;
                }
                "ucinewgame" => {
                    reset_engine_between_games(&mut tt);
                    println!("info string New game started");
                }
                _ => {
                    // Ignora comandos desconhecidos
                }
            }
        }
    }
}

fn test_position(name: &str, fen: &str) {
    println!("📋 {}", name);
    println!("FEN: {}", fen);
    
    match Board::from_fen(fen) {
        Ok(board) => {
            println!("Jogador a mover: {:?}", board.to_move);
            
            // Gera movimentos e mede tempo
            let start = Instant::now();
            let moves = board.generate_all_moves();
            let generation_time = start.elapsed();
            
            println!("✅ Movimentos gerados: {}", moves.len());
            println!("⏱️  Tempo de geração: {:.2}μs", generation_time.as_micros());
            
            // Valida cada movimento
            let start_validation = Instant::now();
            let mut valid_count = 0;
            let mut invalid_moves = Vec::new();
            
            for mv in &moves {
                if is_valid_move(&board, mv) {
                    valid_count += 1;
                } else {
                    invalid_moves.push(*mv);
                }
            }
            let validation_time = start_validation.elapsed();
            
            println!("✅ Movimentos válidos: {}/{}", valid_count, moves.len());
            println!("⏱️  Tempo de validação: {:.2}μs", validation_time.as_micros());
            
            if !invalid_moves.is_empty() {
                println!("⚠️  Movimentos inválidos encontrados:");
                for mv in &invalid_moves {
                    println!("   - {}", mv);
                }
            }
            
            // Mostra alguns movimentos
            if moves.len() > 0 {
                println!("Primeiros movimentos:");
                for (i, mv) in moves.iter().take(5).enumerate() {
                    println!("   {}. {}", i + 1, mv);
                }
                if moves.len() > 5 {
                    println!("   ... e mais {} movimentos", moves.len() - 5);
                }
            }
        }
        Err(e) => {
            println!("❌ Erro ao carregar FEN: {}", e);
        }
    }
}

// Validação básica de movimento
fn is_valid_move(board: &Board, mv: &Move) -> bool {
    let from_bb = 1u64 << mv.from;
    let to_bb = 1u64 << mv.to;
    
    // Verifica se há uma peça nossa na casa de origem
    let our_pieces = if board.to_move == Color::White { 
        board.white_pieces 
    } else { 
        board.black_pieces 
    };
    
    if (our_pieces & from_bb) == 0 {
        return false; // Não há peça nossa na casa de origem
    }
    
    // Verifica se não estamos capturando nossa própria peça
    if (our_pieces & to_bb) != 0 {
        return false;
    }
    
    // Movimento básico válido
    true
}


fn perft_test_parallel(fen: &str, max_depth: u8) {
    match Board::from_fen(fen) {
        Ok(mut board) => {
            let available_cores = num_cpus::get().saturating_sub(1);
            println!("FEN: {}", fen);
            println!("Jogador: {:?}\n", board.to_move);
            println!("Cores disponíveis: {}\n", available_cores);
            
            for depth in 1..=max_depth {
                let start = Instant::now();
                let nodes = perft_parallel(&mut board, depth);
                let elapsed = start.elapsed();
                
                println!("Paralelo Depth {}: {} nós em {:.2}ms ({:.0} nós/seg)", 
                         depth, 
                         nodes, 
                         elapsed.as_millis(),
                         nodes as f64 / elapsed.as_secs_f64());
            }
        }
        Err(e) => {
            println!("❌ Erro ao carregar FEN: {}", e);
        }
    }
}

fn perft_with_tt(board: &mut Board, depth: u8, tt: &mut PerftTT) -> u64 {
    if depth == 0 {
        return 1;
    }

    // Verifica cache primeiro
    if let Some(cached_nodes) = tt.get(board.zobrist_hash, depth) {
        return cached_nodes;
    }

    let moves = board.generate_all_moves(); // pseudo-legais

    if depth == 1 {
        // Bulk counting: Filtra legais sem make/unmake completo
        let nodes = moves.iter()
            .filter(|&&mv| board.is_legal_move(mv))
            .count() as u64;
        tt.insert(board.zobrist_hash, depth, nodes);
        return nodes;
    }

    let mut nodes = 0;

    for mv in moves {
        let undo_info = board.make_move_with_undo(mv);

        let previous_to_move = !board.to_move;
        if !board.is_king_in_check(previous_to_move) {
            nodes += perft_with_tt(board, depth - 1, tt);
        }

        board.unmake_move(mv, undo_info);
    }

    // Cache resultado
    tt.insert(board.zobrist_hash, depth, nodes);
    nodes
}

/// Perft paralelo para alta performance em CPUs multi-core
fn perft_parallel(board: &mut Board, depth: u8) -> u64 {
    if depth <= 2 {
        // Use versão sequencial para profundidades baixas
        return perft_with_tt(board, depth, &mut PerftTT::new());
    }
    
    let moves = board.generate_all_moves();
    
    moves.par_iter().map(|&mv| {
        let mut board_clone = *board; // Copy barato devido ao trait Copy
        let undo_info = board_clone.make_move_with_undo(mv);
        let previous_to_move = !board_clone.to_move;
        
        if !board_clone.is_king_in_check(previous_to_move) {
            perft_with_tt(&mut board_clone, depth - 1, &mut PerftTT::new())
        } else {
            0
        }
    }).sum()
}

// ============================================================================
// HANDLERS DE COMANDOS UCI
// ============================================================================

fn handle_position_command(board: &mut Board, commands: &[&str], _tt: &mut PerftTT) -> u16 {
    let mut moves_played = 0u16;
    
    if commands.len() < 2 {
        return moves_played;
    }
    
    match commands[1] {
        "startpos" => {
            *board = Board::new();
            
            // Processa movimentos se houver
            if let Some(moves_index) = commands.iter().position(|&x| x == "moves") {
                for &move_str in &commands[moves_index + 1..] {
                    if let Some(mv) = parse_uci_move(board, move_str) {
                        if board.make_move(mv) {
                            moves_played += 1;
                        }
                    }
                }
            }
        }
        "fen" => {
            // Reconstrói FEN dos próximos 6 tokens
            if commands.len() >= 8 {
                let fen = commands[2..8].join(" ");
                if let Ok(new_board) = Board::from_fen(&fen) {
                    *board = new_board;
                    
                    // Processa movimentos se houver
                    if let Some(moves_index) = commands.iter().position(|&x| x == "moves") {
                        for &move_str in &commands[moves_index + 1..] {
                            if let Some(mv) = parse_uci_move(board, move_str) {
                                if board.make_move(mv) {
                                    moves_played += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    
    moves_played
}

fn handle_go_command(board: &Board, _tt: &mut PerftTT, _use_opening_book: bool, commands: &[&str], _moves_played: u16, threads: usize) {
    let mut depth = 6; // Profundidade padrão
    let mut time_limit = None;
    
    // Parse dos parâmetros go
    let mut i = 1;
    while i < commands.len() {
        match commands[i] {
            "depth" => {
                if i + 1 < commands.len() {
                    if let Ok(d) = commands[i + 1].parse::<u8>() {
                        depth = d;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < commands.len() {
                    if let Ok(ms) = commands[i + 1].parse::<u64>() {
                        time_limit = Some(Duration::from_millis(ms));
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "wtime" | "btime" => {
                if i + 1 < commands.len() {
                    if let Ok(ms) = commands[i + 1].parse::<u64>() {
                        // Usa 1/30 do tempo disponível para este movimento
                        time_limit = Some(Duration::from_millis(ms / 30));
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    
    // Executa busca com threads configuráveis
    let mut engine = AlphaBetaEngine::new_with_threads(threads);
    let result = if let Some(time) = time_limit {
        engine.search_time(board, time, depth.min(10))
    } else {
        engine.search(board, depth)
    };
    
    // O engine já imprime as linhas de pensamento durante a busca
    
    // Envia melhor movimento
    if let Some(best_move) = result.best_move {
        println!("bestmove {}", format_uci_move(best_move));
    } else {
        println!("bestmove (none)");
    }
}

fn handle_setoption_command(commands: &[&str], use_opening_book: &mut bool, threads: &mut usize) {
    if commands.len() >= 5 && commands[1] == "name" {
        match commands[2] {
            "OwnBook" => {
                if commands.len() >= 5 && commands[3] == "value" {
                    *use_opening_book = commands[4] == "true";
                }
            }
            "Threads" => {
                if commands.len() >= 5 && commands[3] == "value" {
                    if let Ok(t) = commands[4].parse::<usize>() {
                        *threads = t.max(1).min(64); // Limite entre 1-64 threads
                    }
                }
            }
            _ => {}
        }
    }
}

fn reset_engine_between_games(_tt: &mut PerftTT) {
    // Limpa transposition table entre jogos
    *_tt = PerftTT::new();
}

// ============================================================================
// UTILITÁRIOS UCI
// ============================================================================

fn parse_uci_move(board: &Board, move_str: &str) -> Option<Move> {
    if move_str.len() < 4 {
        return None;
    }
    
    let from = parse_square(&move_str[0..2])?;
    let to = parse_square(&move_str[2..4])?;
    
    let promotion = if move_str.len() >= 5 {
        match move_str.chars().nth(4)? {
            'q' => Some(PieceKind::Queen),
            'r' => Some(PieceKind::Rook),
            'b' => Some(PieceKind::Bishop),
            'n' => Some(PieceKind::Knight),
            _ => None,
        }
    } else {
        None
    };
    
    // Verifica se é roque
    let is_castling = (board.kings & (1u64 << from)) != 0 && 
                      ((from == 4 && (to == 6 || to == 2)) || 
                       (from == 60 && (to == 62 || to == 58)));
    
    // Verifica se é en passant
    let is_en_passant = (board.pawns & (1u64 << from)) != 0 &&
                        board.en_passant_target == Some(to) &&
                        (board.white_pieces & (1u64 << to)) == 0 &&
                        (board.black_pieces & (1u64 << to)) == 0;
    
    Some(Move {
        from,
        to,
        promotion,
        is_castling,
        is_en_passant,
    })
}

fn parse_square(square_str: &str) -> Option<u8> {
    if square_str.len() != 2 {
        return None;
    }
    
    let file = square_str.chars().nth(0)? as u8 - b'a';
    let rank = square_str.chars().nth(1)? as u8 - b'1';
    
    if file < 8 && rank < 8 {
        Some(rank * 8 + file)
    } else {
        None
    }
}

fn format_uci_move(mv: Move) -> String {
    let from_file = (mv.from % 8) as u8 + b'a';
    let from_rank = (mv.from / 8) as u8 + b'1';
    let to_file = (mv.to % 8) as u8 + b'a';
    let to_rank = (mv.to / 8) as u8 + b'1';
    
    let mut result = format!("{}{}{}{}", 
                            from_file as char, 
                            from_rank as char,
                            to_file as char, 
                            to_rank as char);
    
    if let Some(promotion) = mv.promotion {
        result.push(match promotion {
            PieceKind::Queen => 'q',
            PieceKind::Rook => 'r',
            PieceKind::Bishop => 'b',
            PieceKind::Knight => 'n',
            _ => 'q',
        });
    }
    
    result
}