// Motor de Xadrez - Teste de Validação e Performance
use pelanca::*;
use std::time::Instant;

fn main() {
    println!("=== TESTE DE VALIDAÇÃO DE MOVIMENTOS ===\n");
    
    // Posições de teste
    let test_positions = [
        ("Posição inicial", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("Posição complexa", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
        ("Final de jogo", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
    ];
    
    for (name, fen) in test_positions.iter() {
        test_position(name, fen);
        println!();
    }
    
    // Teste de performance (perft) na posição inicial
    println!("=== TESTE DE PERFORMANCE (PERFT) ===\n");
    perft_test("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 8);
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

fn perft_test(fen: &str, max_depth: u8) {
    match Board::from_fen(fen) {
        Ok(mut board) => {
            println!("FEN: {}", fen);
            println!("Jogador: {:?}\n", board.to_move);
            
            for depth in 1..=max_depth {
                let start = Instant::now();
                let nodes = perft(&mut board, depth);
                let elapsed = start.elapsed();
                
                println!("Profundidade {}: {} nós em {:.2}ms ({:.0} nós/seg)", 
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

fn perft(board: &mut Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    
    let moves = board.generate_all_moves(); // pseudo-legais
    
    if depth == 1 {
        // Para profundidade 1, filtra apenas os movimentos legais
        return moves.iter()
            .filter(|&&mv| board.is_legal_move(mv))
            .count() as u64;
    }
    
    let mut nodes = 0;
    let mut legal_moves = 0;
    let moves_count = moves.len();
    
    for mv in moves {
        // Aplica o movimento e verifica se é legal (rei não fica em xeque)
        let undo_info = board.make_move_with_undo(mv);
        
        // Verifica se o movimento é legal (rei da cor que jogou não está em xeque)
        let original_color = !board.to_move; // Cor que acabou de jogar (turno já mudou)
        if !board.is_king_in_check(original_color) {
            legal_moves += 1;
            nodes += perft(board, depth - 1);
        }
        
        board.unmake_move(mv, undo_info);
    }
    
    
    nodes
}