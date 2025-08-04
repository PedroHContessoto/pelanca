use crate::core::*;
use crate::search::{*, alpha_beta::{AlphaBetaSearcher, MATE_SCORE}};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Instant;

pub struct ParallelSearcher {
    pub controller: Arc<SearchController>,
    searchers: Vec<AlphaBetaSearcher>,
    best_move: Arc<std::sync::Mutex<Option<Move>>>,
    best_score: Arc<std::sync::Mutex<i16>>,
    total_nodes: Arc<AtomicU64>,
}

impl ParallelSearcher {
    pub fn new(controller: Arc<SearchController>) -> Self {
        let num_threads = controller.config.threads;
        let searchers = (0..num_threads)
            .map(|i| AlphaBetaSearcher::new(controller.clone(), i))
            .collect();

        ParallelSearcher {
            controller,
            searchers,
            best_move: Arc::new(std::sync::Mutex::new(None)),
            best_score: Arc::new(std::sync::Mutex::new(0)),
            total_nodes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn iterative_deepening(&mut self, board: &mut Board) -> (Move, SearchStats) {
        let start_time = Instant::now();
        self.clear_search_data();

        let root_moves = board.generate_legal_moves();
        if root_moves.is_empty() {
            let dummy_move = Move {
                from: 0, to: 0, promotion: None,
                is_castling: false, is_en_passant: false,
            };
            return (dummy_move, SearchStats::default());
        }

        if root_moves.len() == 1 {
            *self.best_move.lock().unwrap() = Some(root_moves[0]);
            return (root_moves[0], SearchStats::default());
        }

        let mut depth_completed = 0u8;

        // Iterative deepening loop
        for depth in 1..=self.controller.config.max_depth {
            if self.controller.should_stop() {
                break;
            }

            let iteration_start = Instant::now();
            
            // Get current best move for aspiration windows
            let current_best = *self.best_score.lock().unwrap();
            let (window_alpha, window_beta) = if depth <= 4 {
                (-MATE_SCORE, MATE_SCORE)
            } else {
                let aspiration_window = 50;
                (current_best - aspiration_window, current_best + aspiration_window)
            };

            // Parallel search with Lazy SMP
            self.lazy_smp_search(board, window_alpha, window_beta, depth);

            if self.controller.should_stop() {
                break;
            }

            depth_completed = depth;

            // Print UCI info
            let nodes = self.total_nodes.load(Ordering::Relaxed);
            let best_score = *self.best_score.lock().unwrap();
            let iteration_time = iteration_start.elapsed();
            let nps = if iteration_time.as_secs_f64() > 0.0 {
                (nodes as f64 / iteration_time.as_secs_f64()) as u64
            } else {
                0
            };

            println!("info depth {} score cp {} nodes {} nps {} time {} pv {}",
                depth,
                best_score,
                nodes,
                nps,
                iteration_time.as_millis(),
                self.format_pv()
            );

            // Stop early if mate found
            if best_score.abs() > MATE_SCORE - 1000 {
                break;
            }
        }

        let final_move = self.best_move.lock().unwrap()
            .unwrap_or(root_moves[0]);
        let stats = self.get_search_stats(start_time, depth_completed);
        
        (final_move, stats)
    }

    fn lazy_smp_search(&mut self, board: &Board, alpha: i16, beta: i16, depth: u8) {
        let num_threads = self.controller.config.threads;
        
        // Clone boards for each thread
        let boards: Vec<Board> = (0..num_threads)
            .map(|_| board.clone())
            .collect();

        // Use crossbeam scope for proper lifetime management
        crossbeam::scope(|s| {
            let handles: Vec<_> = self.searchers
                .iter_mut()
                .zip(boards.into_iter())
                .enumerate()
                .map(|(thread_id, (searcher, mut board))| {
                    let best_move = self.best_move.clone();
                    let best_score = self.best_score.clone();
                    
                    s.spawn(move |_| {
                        // Different search parameters for helper threads (Lazy SMP)
                        let (search_alpha, search_beta, search_depth) = if thread_id == 0 {
                            // Main thread uses full window
                            (alpha, beta, depth)
                        } else {
                            // Helper threads use different depths and slight variations
                            let depth_variation = match thread_id % 4 {
                                1 => depth.saturating_sub(1),
                                2 => depth + 1,
                                3 => depth.saturating_sub(2),
                                _ => depth,
                            };
                            
                            // Slight alpha/beta variations for diversity
                            let alpha_var = alpha + ((thread_id as i16) % 3 - 1) * 10;
                            let beta_var = beta + ((thread_id as i16) % 3 - 1) * 10;
                            
                            (alpha_var, beta_var, depth_variation.max(1))
                        };

                        let score = searcher.search_root(&mut board, search_alpha, search_beta, search_depth);
                        
                        // Update global best if this thread found better move
                        if thread_id == 0 || score > *best_score.lock().unwrap() {
                            if let Some(move_found) = searcher.get_best_move() {
                                *best_move.lock().unwrap() = Some(move_found);
                                *best_score.lock().unwrap() = score;
                            }
                        }
                    })
                })
                .collect();

            // Wait for all threads to complete
            for handle in handles {
                let _ = handle.join();
            }
        }).unwrap();

        // Accumulate node counts
        let total_nodes: u64 = self.searchers
            .iter()
            .map(|s| s.get_nodes())
            .sum();
        
        self.total_nodes.store(total_nodes, Ordering::Relaxed);
    }

    fn clear_search_data(&mut self) {
        *self.best_move.lock().unwrap() = None;
        *self.best_score.lock().unwrap() = 0;
        self.total_nodes.store(0, Ordering::Relaxed);
        for searcher in &mut self.searchers {
            searcher.clear_search_data();
        }
    }

    fn format_pv(&self) -> String {
        if let Some(best_move) = *self.best_move.lock().unwrap() {
            format!("{}", best_move)
        } else {
            "none".to_string()
        }
    }

    fn get_search_stats(&self, start_time: Instant, depth: u8) -> SearchStats {
        let elapsed = start_time.elapsed();
        let nodes = self.total_nodes.load(Ordering::Relaxed);
        let (tt_hits, tt_misses, _, _) = self.controller.tt.stats();
        
        SearchStats {
            nodes_searched: nodes,
            tt_hits,
            tt_misses,
            depth_reached: depth,
            time_elapsed: elapsed,
            nps: if elapsed.as_secs_f64() > 0.0 {
                (nodes as f64 / elapsed.as_secs_f64()) as u64
            } else {
                0
            },
        }
    }
}

pub fn parallel_search(board: &mut Board, controller: Arc<SearchController>) -> (Move, SearchStats) {
    let mut searcher = ParallelSearcher::new(controller);
    searcher.iterative_deepening(board)
}