// Pelanca Mate v3 - Entry Point
//
// Uso:
//   pelanca_mate_v3           -> Modo UCI (para Arena e outras GUIs)
//   pelanca_mate_v3 --bench   -> Modo benchmark (PERFT + busca)

use pelanca::*;
use pelanca::engine::perft::perft_parallel;
use pelanca::engine::eval::PstEvaluator;
use pelanca::engine::search::NegamaxSearcher;
use pelanca::engine::{Searcher, SearchConfig};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--bench") {
        run_bench();
    } else {
        pelanca::uci::run();
    }
}

fn run_bench() {
    println!("=== Pelanca Mate v3 - Benchmark ===\n");

    // PERFT
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

    // Busca
    println!("\n--- Negamax Alpha-Beta (Iterative Deepening) ---");
    let board = Board::new();
    let eval = PstEvaluator;
    let mut searcher = NegamaxSearcher::new(eval);

    let config = SearchConfig { max_depth: 6, ..Default::default() };
    let start = Instant::now();
    let result = searcher.search(&board, &config);
    let elapsed = start.elapsed();

    println!("Depth {}: melhor lance = {}, score = {}, nos = {} ({}ms)",
        result.depth,
        result.best_move.map(|m| m.to_string()).unwrap_or("none".into()),
        result.score,
        result.nodes_searched,
        elapsed.as_millis());

    if !result.pv.is_empty() {
        let pv_str: Vec<String> = result.pv.iter().map(|m| m.to_string()).collect();
        println!("PV: {}", pv_str.join(" "));
    }
}
