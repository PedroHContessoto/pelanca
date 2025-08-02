// Motor de Xadrez - Teste de Validação e Performance
use pelanca::*;
use std::time::Instant;
use std::collections::HashMap;
use rayon::prelude::*;

/// Transposition Table para cache de resultados perft
struct PerftTT {
    table: HashMap<(u64, u8), u64>, // (zobrist_hash, depth) -> nodes
    hits: u64,
    misses: u64,
}

impl PerftTT {
    fn new() -> Self {
        PerftTT {
            table: HashMap::with_capacity(2_000_000), // ~16MB cache
            hits: 0,
            misses: 0,
        }
    }
    
    fn get(&mut self, hash: u64, depth: u8) -> Option<u64> {
        if let Some(&nodes) = self.table.get(&(hash, depth)) {
            self.hits += 1;
            Some(nodes)
        } else {
            self.misses += 1;
            None
        }
    }
    
    fn insert(&mut self, hash: u64, depth: u8, nodes: u64) {
        self.table.insert((hash, depth), nodes);
    }
    
    fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 { 0.0 }
        else { self.hits as f64 / (self.hits + self.misses) as f64 }
    }
}

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
    // println!("=== TESTE DE PERFORMANCE (PERFT COM TT) ===\n");
    // perft_test("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 7);
    
    // Teste sem TT para debugging
    // println!("\n=== TESTE SEM TT (DEBUGGING) ===");
    // perft_test_no_tt("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 7);
    
    // Teste paralelo para profundidades altas
    println!("\n=== TESTE PARALELO (DEPTH 6) ===");
    perft_test_parallel("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 7);
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
            
            let mut tt = PerftTT::new();
            
            for depth in 1..=max_depth {
                let start = Instant::now();
                let nodes = perft_with_tt(&mut board, depth, &mut tt);
                let elapsed = start.elapsed();
                
                println!("Profundidade {}: {} nós em {:.2}ms ({:.0} nós/seg) [TT hit: {:.1}%]", 
                         depth, 
                         nodes, 
                         elapsed.as_millis(),
                         nodes as f64 / elapsed.as_secs_f64(),
                         tt.hit_rate() * 100.0);
            }
            
            println!("\nEstatísticas TT: {} hits, {} misses (hit rate: {:.1}%)",
                     tt.hits, tt.misses, tt.hit_rate() * 100.0);
        }
        Err(e) => {
            println!("❌ Erro ao carregar FEN: {}", e);
        }
    }
}

fn perft_test_no_tt(fen: &str, max_depth: u8) {
    match Board::from_fen(fen) {
        Ok(mut board) => {
            println!("FEN: {}", fen);
            println!("Jogador: {:?}\n", board.to_move);
            
            for depth in 1..=max_depth {
                let start = Instant::now();
                let nodes = perft_no_tt(&mut board, depth);
                let elapsed = start.elapsed();
                
                println!("Profundidade {}: {} nós em {:.2}ms ({:.0} nós/seg) [SEM TT]", 
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

fn perft_test_parallel(fen: &str, max_depth: u8) {
    match Board::from_fen(fen) {
        Ok(mut board) => {
            println!("FEN: {}", fen);
            println!("Jogador: {:?}\n", board.to_move);
            println!("Cores disponíveis: {}\n", num_cpus::get());
            
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

// Versão sem TT para debugging (elimina bugs de cache)
fn perft_no_tt(board: &mut Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    
    let moves = board.generate_all_moves(); // pseudo-legais
    
    if depth == 1 {
        // Bulk counting: conta apenas movimentos legais
        return moves.iter()
            .filter(|&&mv| board.is_legal_move(mv))
            .count() as u64;
    }
    
    let mut nodes = 0;
    
    for mv in moves {
        let undo_info = board.make_move_with_undo(mv);
        
        let previous_to_move = !board.to_move;
        if !board.is_king_in_check(previous_to_move) {
            nodes += perft_no_tt(board, depth - 1);
        }
        
        board.unmake_move(mv, undo_info);
    }
    
    nodes
}

// Versão com TT para compatibilidade
fn perft(board: &mut Board, depth: u8) -> u64 {
    let mut tt = PerftTT::new();
    perft_with_tt(board, depth, &mut tt)
}