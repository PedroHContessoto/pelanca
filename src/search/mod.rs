// Search module - Multi-threaded chess engine with transposition table
// Baseado no motor perft mas otimizado para busca de melhor movimento

pub mod transposition_table;
pub mod evaluation;
pub mod move_ordering;
pub mod quiescence;
pub mod alpha_beta;
pub mod parallel_search;

pub use transposition_table::*;
pub use evaluation::*;
pub use move_ordering::*;
pub use quiescence::*;
pub use alpha_beta::*;
pub use parallel_search::*;

use crate::core::*;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

/// Configuracao de busca
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub max_depth: u8,
    pub max_time: Option<Duration>,
    pub max_nodes: Option<u64>,
    pub threads: usize,
    pub hash_size_mb: usize,
    pub aspiration_window: i16,
    pub aspiration_min_depth: u8,
    pub lazy_smp_depth_variations: Vec<(i8, i8)>, // (base_variation, random_range)
    pub lazy_smp_alpha_beta_range: i16,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_depth: 64,
            max_time: None,
            max_nodes: None,
            threads: num_cpus::get(),
            hash_size_mb: 256,
            aspiration_window: 50,
            aspiration_min_depth: 5,
            lazy_smp_depth_variations: vec![(-1, 2), (1, 2), (-2, 2), (0, 2)],
            lazy_smp_alpha_beta_range: 15,
        }
    }
}

pub struct SearchController {
    pub config: SearchConfig,
    pub tt: Arc<TranspositionTable>,
    pub stop_flag: Arc<AtomicBool>,
    pub start_time: Instant,
}

impl SearchController {
    pub fn new(config: SearchConfig) -> Self {
        SearchController {
            tt: Arc::new(TranspositionTable::new(config.hash_size_mb)),
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

#[derive(Debug, Default, Clone)]
pub struct SearchStats {
    pub nodes_searched: u64,
    pub tt_hits: u64,
    pub tt_misses: u64,
    pub depth_reached: u8,
    pub time_elapsed: Duration,
    pub nps: u64,
}

pub fn search(board: &mut Board, controller: Arc<SearchController>) -> (Move, SearchStats) {
    parallel_search(board, controller)
}