// Ficheiro: src/engine/search/negamax.rs
// Descrição: Busca Negamax com poda Alpha-Beta.

use crate::core::board::Board;
use crate::engine::traits::*;

/// Busca negamax com poda alpha-beta.
/// Baseline limpo e correto para algoritmos de busca.
pub struct NegamaxSearcher<E: Evaluator> {
    evaluator: E,
    nodes_searched: u64,
}

impl<E: Evaluator> NegamaxSearcher<E> {
    pub fn new(evaluator: E) -> Self {
        NegamaxSearcher {
            evaluator,
            nodes_searched: 0,
        }
    }

    fn negamax(&mut self, board: &Board, depth: u8, mut alpha: Score, beta: Score, ply: u8) -> Score {
        self.nodes_searched += 1;

        // Draw por material insuficiente ou regra dos 50 lances
        if board.is_draw_by_insufficient_material() || board.is_draw_by_50_moves() {
            return SCORE_DRAW;
        }

        // Nó folha: avaliação estática
        if depth == 0 {
            return self.evaluator.evaluate(board);
        }

        let moves = board.generate_all_moves();
        let mut best_score = -SCORE_INF;
        let mut has_legal_move = false;

        for mv in moves {
            let mut child = *board; // Board é Copy — clonagem barata
            if !child.make_move(mv) {
                continue; // Movimento ilegal (deixa rei em xeque)
            }

            has_legal_move = true;
            let score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1);

            if score > best_score {
                best_score = score;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                break; // Beta cutoff
            }
        }

        if !has_legal_move {
            // Sem movimentos legais: mate ou stalemate
            if board.is_king_in_check(board.to_move) {
                // Checkmate — preferir mates mais curtos (ply menor = score mais negativo para oponente)
                return -SCORE_MATE + ply as Score;
            } else {
                // Stalemate — empate
                return SCORE_DRAW;
            }
        }

        best_score
    }
}

impl<E: Evaluator> Searcher for NegamaxSearcher<E> {
    fn search(&mut self, board: &Board, config: &SearchConfig) -> SearchResult {
        self.nodes_searched = 0;

        let moves = board.generate_all_moves();
        let mut best_move = None;
        let mut best_score = -SCORE_INF;
        let mut alpha = -SCORE_INF;
        let beta = SCORE_INF;

        for mv in moves {
            let mut child = *board;
            if !child.make_move(mv) {
                continue;
            }

            let score = -self.negamax(&child, config.max_depth - 1, -beta, -alpha, 1);

            if score > best_score {
                best_score = score;
                best_move = Some(mv);
            }
            if score > alpha {
                alpha = score;
            }
        }

        SearchResult {
            best_move,
            score: best_score,
            depth: config.max_depth,
            nodes_searched: self.nodes_searched,
        }
    }

    fn name(&self) -> &str {
        "negamax-ab"
    }
}
