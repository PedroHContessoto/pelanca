// Ficheiro: src/engine/traits.rs
// Descrição: Traits fundamentais para algoritmos de busca e avaliação.

use crate::core::types::Move;
use crate::core::board::Board;

/// Score numérico em centipawns. Positivo = bom para o lado a mover.
pub type Score = i32;

pub const SCORE_INF: Score = 30_000;
pub const SCORE_MATE: Score = 29_000;
pub const SCORE_DRAW: Score = 0;

/// Resultado de uma busca: melhor lance + score + estatísticas.
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: Score,
    pub depth: u8,
    pub nodes_searched: u64,
}

/// Configuração passada a uma busca.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub max_depth: u8,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig { max_depth: 6 }
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
    fn search(&mut self, board: &Board, config: &SearchConfig) -> SearchResult;

    fn name(&self) -> &str {
        "unnamed"
    }
}
