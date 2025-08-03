// Motor de Xadrez UCI - Pelanca Engine para Arena GUI
use pelanca::*;
use pelanca::search::AlphaBetaTTEngine;
use std::io;
use std::time::Duration;

fn main() {
    // Inicializa dependências do motor
    crate::moves::magic_bitboards::init_magic_bitboards();
    
    let mut board = Board::new();
    let mut engine = AlphaBetaTTEngine::new();
    let mut use_opening_book = true;
    let mut moves_played = 0u16;
    let mut threads = num_cpus::get().max(1);

    // Loop principal UCI
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let commands: Vec<&str> = input.trim().split_whitespace().collect();

        if let Some(&command) = commands.get(0) {
            match command {
                "uci" => {
                    println!("id name Pelanca 1.0");
                    println!("id author Pedro Contessoto");
                    println!("option name Hash type spin default 64 min 1 max 1024");
                    println!("option name Threads type spin default {} min 1 max 64", threads);
                    println!("option name OwnBook type check default true");
                    println!("uciok");
                }
                "isready" => {
                    println!("readyok");
                }
                "position" => {
                    moves_played = handle_position_command(&mut board, &commands);
                }
                "go" => {
                    handle_go_command(&board, &mut engine, &commands, moves_played, threads);
                }
                "setoption" => {
                    handle_setoption_command(&commands, &mut use_opening_book, &mut threads);
                }
                "stop" => {
                    // Engine deve parar busca e retornar melhor movimento atual
                    println!("bestmove (none)");
                }
                "quit" => {
                    break;
                }
                "ucinewgame" => {
                    // Reset engine entre jogos
                    engine = AlphaBetaTTEngine::new();
                    println!("info string New game started - TT cleared");
                }
                _ => {
                    // Ignora comandos desconhecidos silenciosamente
                }
            }
        }
    }
}

fn handle_position_command(board: &mut Board, commands: &[&str]) -> u16 {
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

fn handle_go_command(board: &Board, engine: &mut AlphaBetaTTEngine, commands: &[&str], _moves_played: u16, _threads: usize) {
    let mut depth = 15; // Profundidade padrão
    let mut time_limit = None;
    
    // Parse dos parâmetros go
    let mut i = 1;
    while i < commands.len() {
        match commands[i] {
            "depth" => {
                if i + 1 < commands.len() {
                    if let Ok(d) = commands[i + 1].parse::<u8>() {
                        depth = d.min(20); // Limite máximo
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
            "infinite" => {
                depth = 20;
                time_limit = Some(Duration::from_secs(3600)); // 1 hora
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    
    // Executa busca
    let result = if let Some(time) = time_limit {
        engine.search_time(board, time, depth)
    } else {
        engine.search(board, depth)
    };
    
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
                        *threads = t.max(1).min(64);
                    }
                }
            }
            "Hash" => {
                if commands.len() >= 5 && commands[3] == "value" {
                    if let Ok(hash_mb) = commands[4].parse::<usize>() {
                        println!("info string Hash table set to {} MB", hash_mb);
                    }
                }
            }
            _ => {}
        }
    }
}

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