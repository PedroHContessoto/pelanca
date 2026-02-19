// Ficheiro: src/engine/traits.rs
// Descrição: Traits fundamentais para algoritmos de busca e avaliação.

use crate::core::types::Move;
use crate::core::board::Board;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Score numérico em centipawns. Positivo = bom para o lado a mover.
pub type Score = i32;

pub const SCORE_INF: Score = 30_000;
pub const SCORE_MATE: Score = 29_000;
pub const SCORE_DRAW: Score = 0;

/// Info enviada durante busca (para UCI "info" lines).
#[derive(Debug, Clone)]
pub struct SearchInfo {
    pub depth: u8,
    pub score: Score,
    pub is_mate: bool,
    pub mate_in: Option<i32>,
    pub nodes: u64,
    pub time_ms: u64,
    pub nps: u64,
    pub hashfull: u32,
    pub pv: Vec<Move>,
    pub currmove: Option<Move>,
    pub currmovenumber: Option<u32>,
}

/// Resultado de uma busca: melhor lance + score + estatísticas.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub ponder_move: Option<Move>,
    pub score: Score,
    pub depth: u8,
    pub nodes_searched: u64,
    pub pv: Vec<Move>,
}

/// Configuração passada a uma busca.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub max_depth: u8,
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub movestogo: Option<u32>,
    pub movetime: Option<u64>,
    pub infinite: bool,
    pub nodes_limit: Option<u64>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_depth: 64,
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            movetime: None,
            infinite: false,
            nodes_limit: None,
        }
    }
}

/// Trait para avaliação estática de posições.
///
/// Score é do ponto de vista do lado a mover. Positivo = vantagem.
/// Deve ser rápido — chamado em cada nó folha da busca.
pub trait Evaluator: Send + Sync {
    fn evaluate(&self, board: &Board) -> Score;

    fn name(&self) -> &str {
        "unnamed"
    }
}

/// Trait para algoritmos de busca na árvore de jogo.
///
/// Usa um Evaluator internamente e busca a melhor jogada.
pub trait Searcher {
    /// Busca simples (para uso direto, sem UCI).
    fn search(&mut self, board: &Board, config: &SearchConfig) -> SearchResult;

    /// Busca com callback para enviar info (para UCI).
    fn search_with_info(
        &mut self,
        board: &Board,
        config: &SearchConfig,
        _info_cb: &mut dyn FnMut(&SearchInfo),
    ) -> SearchResult {
        // Default: busca normal sem info
        self.search(board, config)
    }

    /// Retorna o stop flag para sinalização externa (ex: UCI "stop").
    fn stop_flag(&self) -> Arc<AtomicBool>;

    fn name(&self) -> &str {
        "unnamed"
    }
}
