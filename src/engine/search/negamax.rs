// Ficheiro: src/engine/search/negamax.rs
// Descrição: Busca Negamax com poda Alpha-Beta + Iterative Deepening.

use crate::core::board::Board;
use crate::core::types::{Color, Move};
use crate::engine::traits::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Busca negamax com poda alpha-beta e iterative deepening.
pub struct NegamaxSearcher<E: Evaluator> {
    evaluator: E,
    nodes_searched: u64,
    stop: Arc<AtomicBool>,
    start_time: Instant,
    time_limit_ms: u64,
}

impl<E: Evaluator> NegamaxSearcher<E> {
    pub fn new(evaluator: E) -> Self {
        NegamaxSearcher {
            evaluator,
            nodes_searched: 0,
            stop: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            time_limit_ms: u64::MAX,
        }
    }

    /// Verifica se deve parar a busca (tempo ou stop externo).
    #[inline(always)]
    fn should_stop(&self) -> bool {
        // Checar a cada 2048 nós para mínimo overhead
        if self.nodes_searched & 2047 != 0 {
            return false;
        }
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        self.start_time.elapsed().as_millis() as u64 >= self.time_limit_ms
    }

    /// Calcula tempo alocado para este lance.
    fn calculate_time(&self, config: &SearchConfig, color: Color) -> u64 {
        if let Some(movetime) = config.movetime {
            return movetime;
        }
        if config.infinite {
            return u64::MAX;
        }
        if config.wtime.is_none() && config.btime.is_none() {
            return u64::MAX;
        }

        let our_time = match color {
            Color::White => config.wtime.unwrap_or(60000),
            Color::Black => config.btime.unwrap_or(60000),
        };
        let our_inc = match color {
            Color::White => config.winc.unwrap_or(0),
            Color::Black => config.binc.unwrap_or(0),
        };
        let moves_left = config.movestogo.unwrap_or(30) as u64;

        let alloc = our_time / moves_left + our_inc / 2;
        alloc.min(our_time / 2) // Nunca usar mais de 50% do tempo restante
    }

    /// Busca raiz para uma profundidade específica.
    fn search_root(&mut self, board: &Board, depth: u8) -> SearchResult {
        let moves = board.generate_all_moves();
        let mut best_move = None;
        let mut best_score = -SCORE_INF;
        let mut alpha = -SCORE_INF;
        let beta = SCORE_INF;
        let mut best_pv = Vec::new();

        for mv in moves {
            let mut child = *board;
            if !child.make_move(mv) {
                continue;
            }

            let mut child_pv = Vec::new();
            let score = -self.negamax(&child, depth - 1, -beta, -alpha, 1, &mut child_pv);

            if self.should_stop() {
                break;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(mv);
                best_pv.clear();
                best_pv.push(mv);
                best_pv.extend_from_slice(&child_pv);
            }
            if score > alpha {
                alpha = score;
            }
        }

        let ponder_move = if best_pv.len() > 1 {
            Some(best_pv[1])
        } else {
            None
        };

        SearchResult {
            best_move,
            ponder_move,
            score: best_score,
            depth,
            nodes_searched: self.nodes_searched,
            pv: best_pv,
        }
    }

    fn negamax(
        &mut self,
        board: &Board,
        depth: u8,
        mut alpha: Score,
        beta: Score,
        ply: u8,
        pv: &mut Vec<Move>,
    ) -> Score {
        self.nodes_searched += 1;

        if self.should_stop() {
            return 0;
        }

        if board.is_draw_by_insufficient_material() || board.is_draw_by_50_moves() {
            return SCORE_DRAW;
        }

        if depth == 0 {
            return self.evaluator.evaluate(board);
        }

        let moves = board.generate_all_moves();
        let mut best_score = -SCORE_INF;
        let mut has_legal_move = false;

        for mv in moves {
            let mut child = *board;
            if !child.make_move(mv) {
                continue;
            }

            has_legal_move = true;
            let mut child_pv = Vec::new();
            let score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, &mut child_pv);

            if self.should_stop() {
                return 0;
            }

            if score > best_score {
                best_score = score;
                pv.clear();
                pv.push(mv);
                pv.extend_from_slice(&child_pv);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                break;
            }
        }

        if !has_legal_move {
            if board.is_king_in_check(board.to_move) {
                return -SCORE_MATE + ply as Score;
            } else {
                return SCORE_DRAW;
            }
        }

        best_score
    }
}

impl<E: Evaluator> Searcher for NegamaxSearcher<E> {
    fn search(&mut self, board: &Board, config: &SearchConfig) -> SearchResult {
        // Delegate to search_with_info with a no-op callback
        let mut noop = |_: &SearchInfo| {};
        self.search_with_info(board, config, &mut noop)
    }

    fn search_with_info(
        &mut self,
        board: &Board,
        config: &SearchConfig,
        info_cb: &mut dyn FnMut(&SearchInfo),
    ) -> SearchResult {
        self.nodes_searched = 0;
        self.start_time = Instant::now();
        self.stop.store(false, Ordering::Relaxed);
        self.time_limit_ms = self.calculate_time(config, board.to_move);

        let mut best_result = SearchResult {
            best_move: None,
            ponder_move: None,
            score: 0,
            depth: 0,
            nodes_searched: 0,
            pv: Vec::new(),
        };

        let max_depth = config.max_depth.min(64);

        for depth in 1..=max_depth {
            let result = self.search_root(board, depth);

            if self.should_stop() && depth > 1 {
                break;
            }

            let elapsed = self.start_time.elapsed().as_millis() as u64;
            let nps = if elapsed > 0 {
                result.nodes_searched * 1000 / elapsed
            } else {
                result.nodes_searched * 1000
            };

            // Detectar mate score
            let (is_mate, mate_in) = if result.score.abs() > SCORE_MATE - 100 {
                let mate_dist = ((SCORE_MATE - result.score.abs()) + 1) / 2;
                let sign = if result.score > 0 { 1 } else { -1 };
                (true, Some(sign * mate_dist as i32))
            } else {
                (false, None)
            };

            info_cb(&SearchInfo {
                depth,
                score: result.score,
                is_mate,
                mate_in,
                nodes: result.nodes_searched,
                time_ms: elapsed,
                nps,
                pv: result.pv.clone(),
                currmove: None,
                currmovenumber: None,
            });

            best_result = result;

            if self.should_stop() {
                break;
            }
        }

        best_result
    }

    fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    fn name(&self) -> &str {
        "negamax-ab"
    }
}
