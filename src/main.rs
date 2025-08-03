// Motor de Xadrez - Teste Alpha-Beta com TT
use pelanca::*;
use std::time::{Instant, Duration};

use pelanca::search::AlphaBetaTTEngine;

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

    // Teste Alpha-Beta
    println!("\n=== TESTE ALPHA-BETA ===");
    test_alpha_beta();
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

fn test_alpha_beta() {
    println!("Testando Alpha-Beta com TT e multi-core...\n");
    
    let test_positions = [
        ("Posição inicial", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("Posição tática", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
    ];
    
    for (name, fen) in test_positions.iter() {
        println!("🎯 {}", name);
        
        match Board::from_fen(fen) {
            Ok(board) => {
                println!("FEN: {}", fen);
                println!("Jogador: {:?}", board.to_move);
                
                // Teste busca rápida (depth 6)
                let mut engine = AlphaBetaTTEngine::new();
                let result = engine.search(&board, 6);
                
                if let Some(best_move) = result.best_move {
                    println!("✅ Melhor movimento: {}", format_move_simple(best_move));
                    println!("   Score: {} centipawns", result.score);
                    println!("   Nodes: {}", result.nodes_searched);
                    println!("   Tempo: {:.2}ms", result.time_elapsed.as_millis());
                    println!("   NPS: {:.0}", result.nodes_searched as f64 / result.time_elapsed.as_secs_f64());
                } else {
                    println!("❌ Nenhum movimento encontrado");
                }
            }
            Err(e) => {
                println!("❌ Erro ao carregar FEN: {}", e);
            }
        }
        
        println!();
    }
}

fn format_move_simple(mv: Move) -> String {
    let from_file = (mv.from % 8) as u8 + b'a';
    let from_rank = (mv.from / 8) as u8 + b'1';
    let to_file = (mv.to % 8) as u8 + b'a';
    let to_rank = (mv.to / 8) as u8 + b'1';
    
    format!("{}{}{}{}", 
            from_file as char, 
            from_rank as char,
            to_file as char, 
            to_rank as char)
}