// Pelanca Chess Engine - Entry Point
use pelanca::*;
use pelanca::engine::perft::perft_parallel;
use pelanca::engine::eval::MaterialEvaluator;
use pelanca::engine::search::NegamaxSearcher;
use pelanca::engine::{Searcher, SearchConfig};
use std::time::Instant;

fn main() {
    println!("=== Pelanca Chess Engine ===\n");

    // === PERFT ===
    println!("--- PERFT (posicao inicial) ---");
    let mut board = Board::from_fen(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    ).unwrap();

    let cores = num_cpus::get();
    println!("Cores disponiveis: {}\n", cores);

    for depth in 1..=7 {
        let start = Instant::now();
        let nodes = perft_parallel(&mut board, depth);
        let elapsed = start.elapsed();
        println!("Depth {}: {} nos em {}ms ({:.0} nos/seg)",
            depth, nodes,
            elapsed.as_millis(),
            nodes as f64 / elapsed.as_secs_f64());
    }

    // === BUSCA ===
    println!("\n--- Negamax Alpha-Beta ---");
    let board = Board::new();
    let eval = MaterialEvaluator;
    let mut searcher = NegamaxSearcher::new(eval);

    for depth in 1..=5 {
        let config = SearchConfig { max_depth: depth };
        let start = Instant::now();
        let result = searcher.search(&board, &config);
        let elapsed = start.elapsed();

        println!("Depth {}: melhor lance = {}, score = {}, nos = {} ({}ms)",
            depth,
            result.best_move.map(|m| m.to_string()).unwrap_or("none".into()),
            result.score,
            result.nodes_searched,
            elapsed.as_millis());
    }
}
