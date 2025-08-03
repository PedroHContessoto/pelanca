// Sistema de busca alpha-beta de alta performance

pub mod alpha_beta;
pub mod evaluation;
pub mod move_ordering;
pub mod transposition_table;
pub mod search_thread;
pub mod quiescence;


pub use alpha_beta::*;
pub use evaluation::*;
pub use move_ordering::*;
pub use transposition_table::*;
pub use search_thread::*;
pub use quiescence::*;

use crate::core::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configurações globais da busca
pub struct SearchConfig {
    pub max_depth: u8,
    pub max_time: Option<Duration>,
    pub max_nodes: Option<u64>,
    pub num_threads: usize,
    pub use_quiescence: bool,
    pub aspiration_window: i32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_depth: 64,
            max_time: None,
            max_nodes: None,
            num_threads: num_cpus::get().saturating_sub(1).max(1),
            use_quiescence: true,
            aspiration_window: 50,
        }
    }
}

/// Estatísticas da busca
#[derive(Debug, Clone)]
pub struct SearchStats {
    pub nodes_searched: u64,
    pub quiescence_nodes: u64,
    pub tt_hits: u64,
    pub tt_misses: u64,
    pub beta_cutoffs: u64,
    pub depth_reached: u8,
    pub elapsed_time: Duration,
    pub principal_variation: Vec<Move>,
}

impl SearchStats {
    pub fn new() -> Self {
        SearchStats {
            nodes_searched: 0,
            quiescence_nodes: 0,
            tt_hits: 0,
            tt_misses: 0,
            beta_cutoffs: 0,
            depth_reached: 0,
            elapsed_time: Duration::new(0, 0),
            principal_variation: Vec::new(),
        }
    }

    pub fn nodes_per_second(&self) -> u64 {
        if self.elapsed_time.as_secs_f64() > 0.0 {
            (self.nodes_searched as f64 / self.elapsed_time.as_secs_f64()) as u64
        } else {
            0
        }
    }
}

/// Controlador principal da busca
pub struct SearchController {
    pub config: SearchConfig,
    pub stop_flag: Arc<AtomicBool>,
    pub node_counter: Arc<AtomicU64>,
    pub tt: Arc<TranspositionTable>,
    pub start_time: Instant,
}

impl SearchController {
    pub fn new(config: SearchConfig) -> Self {
        SearchController {
            config,
            stop_flag: Arc::new(AtomicBool::new(false)),
            node_counter: Arc::new(AtomicU64::new(0)),
            tt: Arc::new(TranspositionTable::new(256)), // 256MB padrão
            start_time: Instant::now(),
        }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    pub fn should_stop(&self) -> bool {
        // Verifica flag de parada
        if self.stop_flag.load(Ordering::Relaxed) {
            return true;
        }

        // Verifica limite de tempo
        if let Some(max_time) = self.config.max_time {
            if self.start_time.elapsed() >= max_time {
                return true;
            }
        }

        // Verifica limite de nós
        if let Some(max_nodes) = self.config.max_nodes {
            if self.node_counter.load(Ordering::Relaxed) >= max_nodes {
                return true;
            }
        }

        false
    }

    pub fn increment_nodes(&self, count: u64) {
        self.node_counter.fetch_add(count, Ordering::Relaxed);
    }

    pub fn get_elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Tipo de nó na árvore de busca
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    PV,    // Principal Variation
    Cut,   // Cut node (fail-high)
    All,   // All node (fail-low)
}

/// Constantes de avaliação
pub const MATE_SCORE: i32 = 30000;
pub const MATE_THRESHOLD: i32 = MATE_SCORE - 1000;
pub const DRAW_SCORE: i32 = 0;
pub const INF: i32 = 32000;

/// Verifica se um score indica mate
pub fn is_mate_score(score: i32) -> bool {
    score.abs() >= MATE_THRESHOLD
}

/// Ajusta score de mate pela profundidade
pub fn mate_score_adjustment(score: i32, ply: i32) -> i32 {
    if score > MATE_THRESHOLD {
        score - ply
    } else if score < -MATE_THRESHOLD {
        score + ply
    } else {
        score
    }
}