// Search module - Multi-threaded chess engine with transposition table
// Baseado no motor perft mas otimizado para busca de melhor movimento

pub mod transposition_table;
pub mod evaluation;
pub mod move_ordering;
pub mod quiescence;
pub mod alpha_beta;
pub mod search_thread;

pub use transposition_table::*;
pub use evaluation::*;
pub use move_ordering::*;
pub use quiescence::*;
pub use alpha_beta::*;
pub use search_thread::*;

use crate::core::*;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

/// Configuracao de busca
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub max_depth: u8,
    pub max_time: Option<Duration>,
    pub max_nodes: Option<u64>,
    pub threads: usize,
    pub hash_size_mb: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_depth: 64,
            max_time: None,
            max_nodes: None,
            threads: num_cpus::get(),
            hash_size_mb: 256,
        }
    }
}

/// Controlador de busca para coordenar threads e comunicacao
pub struct SearchController {
    pub config: SearchConfig,
    pub tt: Arc<Mutex<TranspositionTable>>,
    pub stop_flag: Arc<AtomicBool>,
    pub start_time: Instant,
}

impl SearchController {
    pub fn new(config: SearchConfig) -> Self {
        SearchController {
            tt: Arc::new(Mutex::new(TranspositionTable::new(config.hash_size_mb))),
            stop_flag: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            config,
        }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    pub fn should_stop(&self) -> bool {
        if self.stop_flag.load(Ordering::Relaxed) {
            return true;
        }

        if let Some(max_time) = self.config.max_time {
            if self.start_time.elapsed() >= max_time {
                self.stop_flag.store(true, Ordering::Relaxed);
                return true;
            }
        }

        false
    }
}

/// Estatisticas de busca
#[derive(Debug, Default, Clone)]
pub struct SearchStats {
    pub nodes_searched: u64,
    pub tt_hits: u64,
    pub tt_misses: u64,
    pub depth_reached: u8,
    pub time_elapsed: Duration,
    pub nps: u64, // Nodes per second
}

/// Funcao principal de busca multi-threaded
pub fn search(board: &mut Board, controller: Arc<SearchController>) -> (Move, SearchStats) {
    let mut searcher = AlphaBetaSearcher::new(controller.clone());
    searcher.iterative_deepening(board)
}