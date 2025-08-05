use crate::core::*;
use crate::search::{*, alpha_beta::{AlphaBetaSearcher, MATE_SCORE}};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Instant;
use rand::{thread_rng, Rng};

pub struct ParallelSearcher {
    pub controller: Arc<SearchController>,
    searchers: Vec<AlphaBetaSearcher>,
    thread_boards: Vec<Board>,
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
            thread_boards: Vec::with_capacity(num_threads),
            best_move: Arc::new(std::sync::Mutex::new(None)),
            best_score: Arc::new(std::sync::Mutex::new(0)),
            total_nodes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn iterative_deepening(&mut self, board: &mut Board) -> (Move, SearchStats) {
        let start_time = Instant::now();
        self.clear_search_data();

        // Check for draw conditions
        if board.is_draw_by_50_moves() || board.is_draw_by_insufficient_material() {
            let dummy_move = Move {
                from: 0, to: 0, promotion: None,
                is_castling: false, is_en_passant: false,
            };
            return (dummy_move, SearchStats::default());
        }

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
            let (mut window_alpha, mut window_beta) = if depth < self.controller.config.aspiration_min_depth {
                (-MATE_SCORE, MATE_SCORE)
            } else {
                let aspiration_window = self.controller.config.aspiration_window;
                (current_best - aspiration_window, current_best + aspiration_window)
            };

            // Aspiration window search with re-search on fail-high/fail-low
            loop {
                // Clear previous iteration's best score for this depth
                *self.best_score.lock().unwrap() = window_alpha;
                
                // Parallel search with Lazy SMP
                self.lazy_smp_search(board, window_alpha, window_beta, depth);
                
                if self.controller.should_stop() {
                    break;
                }
                
                let final_score = *self.best_score.lock().unwrap();
                
                // Check if we need to re-search with wider windows
                if final_score <= window_alpha {
                    // Fail-low: widen alpha window
                    window_alpha = -MATE_SCORE;
                } else if final_score >= window_beta {
                    // Fail-high: widen beta window  
                    window_beta = MATE_SCORE;
                } else {
                    // Score is within window, we're done
                    break;
                }
            }

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
                self.format_pv_with_board(board)
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

    fn prepare_thread_boards(&mut self, board: &Board) {
        let num_threads = self.controller.config.threads;
        self.thread_boards.clear();
        self.thread_boards.reserve(num_threads);
        
        for _ in 0..num_threads {
            self.thread_boards.push(board.clone());
        }
    }

    fn lazy_smp_search(&mut self, board: &Board, alpha: i16, beta: i16, depth: u8) {
        // Prepare boards for each thread (avoiding allocation in hot path)
        self.prepare_thread_boards(board);

        // Clone needed configuration to avoid borrowing issues
        let variations = self.controller.config.lazy_smp_depth_variations.clone();
        let alpha_beta_range = self.controller.config.lazy_smp_alpha_beta_range;

        // Use crossbeam scope for proper lifetime management
        crossbeam::scope(|s| {
            let handles: Vec<_> = self.searchers
                .iter_mut()
                .zip(self.thread_boards.iter_mut())
                .enumerate()
                .map(|(thread_id, (searcher, board))| {
                    let best_move = self.best_move.clone();
                    let best_score = self.best_score.clone();
                    let total_nodes = self.total_nodes.clone();
                    let variations = variations.clone();
                    
                    s.spawn(move |_| {
                        // Different search parameters for helper threads (Lazy SMP)
                        let (search_alpha, search_beta, search_depth) = if thread_id == 0 {
                            // Main thread uses full window
                            (alpha, beta, depth)
                        } else {
                            // Helper threads use different depths with randomness for diversity
                            let mut rng = thread_rng();
                            
                            // Get variation parameters based on thread_id
                            let variation_idx = (thread_id - 1) % variations.len();
                            let (base_var, random_range) = variations[variation_idx];
                            
                            // Apply base variation
                            let base_depth = if base_var >= 0 {
                                depth + base_var as u8
                            } else {
                                depth.saturating_sub((-base_var) as u8)
                            };
                            
                            // Add random perturbation
                            let random_offset = rng.gen_range(-random_range..=random_range);
                            let depth_variation = (base_depth as i8 + random_offset).max(1) as u8;
                            
                            // Random alpha/beta variations for diversity
                            let alpha_rand = rng.gen_range(-alpha_beta_range..=alpha_beta_range);
                            let beta_rand = rng.gen_range(-alpha_beta_range..=alpha_beta_range);
                            let alpha_var = alpha + alpha_rand;
                            let beta_var = beta + beta_rand;
                            
                            (alpha_var, beta_var, depth_variation)
                        };

                        let score = searcher.search_root(board, search_alpha, search_beta, search_depth);
                        
                        // Atomically add this thread's node count to the global total
                        let thread_nodes = searcher.get_nodes();
                        total_nodes.fetch_add(thread_nodes, Ordering::Relaxed);
                        
                        // Update global best if this thread found better or equal move
                        // Prefer main thread for equal scores, otherwise prefer newer equal scores for diversity
                        let should_update = {
                            let current_best = *best_score.lock().unwrap();
                            thread_id == 0 || score > current_best || 
                            (score == current_best && thread_id > 0)
                        };
                        
                        if should_update {
                            if let Some(move_found) = searcher.get_best_move() {
                                // Lock once for both updates to ensure atomicity
                                let mut best_move_guard = best_move.lock().unwrap();
                                let mut best_score_guard = best_score.lock().unwrap();
                                
                                // Double-check after acquiring locks (avoid race conditions)
                                if thread_id == 0 || score >= *best_score_guard {
                                    *best_move_guard = Some(move_found);
                                    *best_score_guard = score;
                                }
                            }
                        }
                    })
                })
                .collect();

            // Wait for all threads to complete
            for (i, handle) in handles.into_iter().enumerate() {
                match handle.join() {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("Warning: Thread {} panicked: {:?}", i, e);
                        // Continue with other threads
                    }
                }
            }
        }).unwrap();
    }

    fn clear_search_data(&mut self) {
        *self.best_move.lock().unwrap() = None;
        *self.best_score.lock().unwrap() = 0;
        self.total_nodes.store(0, Ordering::Relaxed);
        for searcher in &mut self.searchers {
            searcher.clear_search_data();
        }
    }

    fn extract_pv_from_tt(&self, board: &mut Board, depth: u8) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut current_board = board.clone();
        
        for _ in 0..depth {
            // Stop if we detect a draw or repetition
            if current_board.is_draw_by_50_moves() || current_board.is_draw_by_insufficient_material() {
                break;
            }
            
            if let Some(tt_entry) = self.controller.tt.probe(current_board.zobrist_hash) {
                let mv = tt_entry.get_move();
                
                // Verify the move is legal
                let legal_moves = current_board.generate_legal_moves();
                if legal_moves.contains(&mv) {
                    pv.push(mv);
                    
                    // Make the move and continue
                    let undo_info = current_board.make_move_with_undo(mv);
                    
                    // Check if this leads to a legal position  
                    if current_board.is_king_in_check(!current_board.to_move) {
                        current_board.unmake_move(mv, undo_info);
                        break;
                    }
                    
                    // Stop if we've seen this position before (simple repetition detection)
                    let current_hash = current_board.zobrist_hash;
                    if pv.len() > 2 {
                        let mut temp_board = board.clone();
                        let mut seen_hashes = std::collections::HashSet::new();
                        
                        for &prev_mv in &pv[..pv.len()-1] {
                            let _temp_undo = temp_board.make_move_with_undo(prev_mv);
                            if !temp_board.is_king_in_check(!temp_board.to_move) {
                                if seen_hashes.contains(&temp_board.zobrist_hash) || temp_board.zobrist_hash == current_hash {
                                    current_board.unmake_move(mv, undo_info);
                                    return pv[..pv.len()-1].to_vec(); // Remove the repeating move
                                }
                                seen_hashes.insert(temp_board.zobrist_hash);
                            }
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        pv
    }
    
    fn format_pv_with_board(&self, board: &Board) -> String {
        if let Some(best_move) = *self.best_move.lock().unwrap() {
            // Try to extract full PV from transposition table
            let mut board_copy = board.clone();
            let pv = self.extract_pv_from_tt(&mut board_copy, 10); // Extract up to 10 moves
            
            if pv.len() > 1 {
                return pv.iter()
                    .map(|mv| mv.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            
            // Fallback to just the best move
            format!("{}", best_move)
        } else {
            "none".to_string()
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