// Search Thread - Implementação multi-threaded para busca paralela
// Coordena��o de threads usando Lazy SMP (Lazy Symmetric Multi-Processing)

use crate::core::*;
use crate::search::{*, alpha_beta::AlphaBetaSearcher};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use rayon::prelude::*;

/// Dados compartilhados entre threads
pub struct SharedSearchData {
    pub best_move: Arc<Mutex<Option<Move>>>,
    pub best_score: Arc<Mutex<i16>>,
    pub nodes_searched: Arc<AtomicU64>,
    pub depth_completed: Arc<Mutex<u8>>,
    pub principal_variation: Arc<Mutex<Vec<Move>>>,
}

impl SharedSearchData {
    pub fn new() -> Self {
        SharedSearchData {
            best_move: Arc::new(Mutex::new(None)),
            best_score: Arc::new(Mutex::new(0)),
            nodes_searched: Arc::new(AtomicU64::new(0)),
            depth_completed: Arc::new(Mutex::new(0)),
            principal_variation: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn update_best_move(&self, mv: Move, score: i16, depth: u8, pv: Vec<Move>) {
        if let Ok(mut best_move) = self.best_move.try_lock() {
            *best_move = Some(mv);
        }
        if let Ok(mut best_score) = self.best_score.try_lock() {
            *best_score = score;
        }
        if let Ok(mut depth_completed) = self.depth_completed.try_lock() {
            *depth_completed = depth;
        }
        if let Ok(mut principal_variation) = self.principal_variation.try_lock() {
            *principal_variation = pv;
        }
    }

    pub fn add_nodes(&self, nodes: u64) {
        self.nodes_searched.fetch_add(nodes, Ordering::Relaxed);
    }

    pub fn get_nodes(&self) -> u64 {
        self.nodes_searched.load(Ordering::Relaxed)
    }
}

/// Coordenador de busca multi-threaded
pub struct ParallelSearchCoordinator {
    pub controller: Arc<SearchController>,
    pub shared_data: Arc<SharedSearchData>,
    thread_count: usize,
}

impl ParallelSearchCoordinator {
    pub fn new(controller: Arc<SearchController>) -> Self {
        let thread_count = controller.config.threads.max(1);
        
        ParallelSearchCoordinator {
            controller,
            shared_data: Arc::new(SharedSearchData::new()),
            thread_count,
        }
    }

    /// Busca paralela usando Lazy SMP
    pub fn search_parallel(&self, board: &Board) -> (Move, SearchStats) {
        let start_time = Instant::now();
        
        // Thread principal (master) faz busca normal
        let master_board = *board;
        let master_controller = self.controller.clone();
        let master_shared = self.shared_data.clone();
        
        // Threads auxiliares fazem buscas ligeiramente diferentes
        let helper_threads: Vec<_> = (1..self.thread_count)
            .map(|thread_id| {
                let board_copy = *board;
                let controller_copy = self.controller.clone();
                let shared_copy = self.shared_data.clone();
                
                thread::spawn(move || {
                    Self::helper_thread_search(board_copy, controller_copy, shared_copy, thread_id)
                })
            })
            .collect();

        // Thread principal
        let (best_move, master_stats) = self.master_thread_search(master_board, master_controller, master_shared);

        // Aguarda threads auxiliares
        for handle in helper_threads {
            let _ = handle.join();
        }

        // Coleta estat�sticas finais
        let final_stats = SearchStats {
            nodes_searched: self.shared_data.get_nodes(),
            depth_reached: *self.shared_data.depth_completed.lock().unwrap(),
            time_elapsed: start_time.elapsed(),
            nps: if start_time.elapsed().as_secs_f64() > 0.0 {
                (self.shared_data.get_nodes() as f64 / start_time.elapsed().as_secs_f64()) as u64
            } else {
                0
            },
            ..master_stats
        };

        (best_move, final_stats)
    }

    /// Busca da thread principal (iterative deepening normal)
    fn master_thread_search(
        &self,
        mut board: Board,
        controller: Arc<SearchController>,
        shared_data: Arc<SharedSearchData>,
    ) -> (Move, SearchStats) {
        let mut searcher = AlphaBetaSearcher::new(controller.clone());
        let (best_move, stats) = searcher.iterative_deepening(&mut board);
        
        // Atualiza dados compartilhados
        shared_data.add_nodes(stats.nodes_searched);
        
        (best_move, stats)
    }

    /// Busca das threads auxiliares (com varia��es para diversidade)
    fn helper_thread_search(
        mut board: Board,
        controller: Arc<SearchController>,
        shared_data: Arc<SharedSearchData>,
        thread_id: usize,
    ) {
        let mut searcher = AlphaBetaSearcher::new(controller.clone());
        
        // Lazy SMP: cada thread faz busca ligeiramente diferente
        let depth_offset = (thread_id % 4) as u8; // Varia profundidade inicial
        let start_depth = 1 + depth_offset;
        
        // Busca com profundidade iterativa come�ando em ponto diferente
        for depth in start_depth..=controller.config.max_depth {
            if controller.should_stop() {
                break;
            }

            // Aspiration window ligeiramente diferente para cada thread
            let window_size = 50 + (thread_id as i16 * 10);
            let previous_score = *shared_data.best_score.lock().unwrap();
            
            let (alpha, beta) = if depth <= 4 {
                (-30000, 30000) // Full window para profundidades baixas
            } else {
                (previous_score - window_size, previous_score + window_size)
            };

            let score = searcher.alpha_beta_root(&mut board, alpha, beta, depth, thread_id as u16);
            
            // Se busca foi completa, atualiza dados compartilhados
            if !controller.should_stop() {
                if let Some(best_move) = searcher.get_best_move() {
                    shared_data.update_best_move(best_move, score, depth, Vec::new());
                }
                shared_data.add_nodes(searcher.get_nodes_searched());
            }
        }
    }
}

/// Extens�o do AlphaBetaSearcher para root search multi-threaded
impl AlphaBetaSearcher {
    /// Busca root espec�fica para threads auxiliares
    pub fn alpha_beta_root(
        &mut self,
        board: &mut Board,
        mut alpha: i16,
        beta: i16,
        depth: u8,
        thread_id: u16,
    ) -> i16 {
        if self.controller.should_stop() {
            return 0;
        }

        let moves = board.generate_all_moves();
        if moves.is_empty() {
            return if board.is_king_in_check(board.to_move) {
                -30000 + thread_id as i16 // Mate
            } else {
                0 // Stalemate
            };
        }

        let mut ordered_moves = moves;
        let tt_move = if let Ok(tt) = self.controller.tt.lock() {
            tt.probe(board.zobrist_hash).map(|entry| entry.best_move)
        } else {
            None
        };
        
        self.move_orderer.order_moves(board, &mut ordered_moves, tt_move, 0);

        let mut best_score = -30001;
        let mut best_move = ordered_moves[0];

        for (move_index, &mv) in ordered_moves.iter().enumerate() {
            let undo_info = board.make_move_with_undo(mv);
            let previous_to_move = !board.to_move;
            
            if board.is_king_in_check(previous_to_move) {
                board.unmake_move(mv, undo_info);
                continue;
            }

            let score = if move_index == 0 || best_score == -30001 {
                // Primeiro movimento ou ainda n�o encontrou movimento legal
                -self.alpha_beta(board, -beta, -alpha, depth - 1, 1, true)
            } else {
                // Principal Variation Search (PVS)
                let score = -self.alpha_beta(board, -alpha - 1, -alpha, depth - 1, 1, false);
                
                if score > alpha && score < beta {
                    // Re-search com janela completa
                    -self.alpha_beta(board, -beta, -alpha, depth - 1, 1, true)
                } else {
                    score
                }
            };

            board.unmake_move(mv, undo_info);

            if self.controller.should_stop() {
                return best_score;
            }

            if score > best_score {
                best_score = score;
                best_move = mv;
                self.best_move = Some(mv);

                if score > alpha {
                    alpha = score;
                    
                    if score >= beta {
                        // Beta cutoff
                        self.move_orderer.update_history_cutoff(
                            board, 
                            mv, 
                            depth, 
                            &ordered_moves[..move_index]
                        );
                        break;
                    }
                }
            }
        }

        best_score
    }

    /// Obt�m melhor movimento encontrado
    pub fn get_best_move(&self) -> Option<Move> {
        self.best_move
    }

    /// Obt�m n�mero de n�s pesquisados
    pub fn get_nodes_searched(&self) -> u64 {
        self.nodes_searched
    }
}

/// Fun��o utilit�ria para busca paralela simplificada
pub fn parallel_search(board: &Board, controller: Arc<SearchController>) -> (Move, SearchStats) {
    let coordinator = ParallelSearchCoordinator::new(controller);
    coordinator.search_parallel(board)
}

/// Implementa��o alternativa usando Rayon para work-stealing
pub struct RayonSearchCoordinator {
    controller: Arc<SearchController>,
}

impl RayonSearchCoordinator {
    pub fn new(controller: Arc<SearchController>) -> Self {
        RayonSearchCoordinator { controller }
    }

    /// Busca paralela usando Rayon (work-stealing)
    pub fn search_rayon(&self, board: &Board) -> (Move, SearchStats) {
        let start_time = Instant::now();
        let moves = board.generate_all_moves();
        
        if moves.is_empty() {
            let dummy_move = Move {
                from: 0, to: 0, promotion: None,
                is_castling: false, is_en_passant: false,
            };
            return (dummy_move, SearchStats::default());
        }

        // Busca paralela nos primeiros movimentos
        let results: Vec<_> = moves.par_iter()
            .take(self.controller.config.threads.min(moves.len()))
            .map(|&mv| {
                let mut board_copy = *board;
                let undo_info = board_copy.make_move_with_undo(mv);
                let previous_to_move = !board_copy.to_move;
                
                if board_copy.is_king_in_check(previous_to_move) {
                    board_copy.unmake_move(mv, undo_info);
                    return (mv, -30001, 0u64); // Movimento ilegal
                }

                let mut searcher = AlphaBetaSearcher::new(self.controller.clone());
                let score = -searcher.alpha_beta(
                    &mut board_copy, 
                    -30000, 
                    30000, 
                    self.controller.config.max_depth.saturating_sub(1), 
                    1, 
                    true
                );
                
                board_copy.unmake_move(mv, undo_info);
                (mv, score, searcher.nodes_searched)
            })
            .collect();

        // Encontra melhor resultado
        let (best_move, best_score, total_nodes) = results.into_iter()
            .filter(|(_, score, _)| *score > -30001) // Remove movimentos ilegais
            .max_by_key(|(_, score, _)| *score)
            .unwrap_or((moves[0], 0, 0));

        let stats = SearchStats {
            nodes_searched: total_nodes,
            depth_reached: self.controller.config.max_depth,
            time_elapsed: start_time.elapsed(),
            nps: if start_time.elapsed().as_secs_f64() > 0.0 {
                (total_nodes as f64 / start_time.elapsed().as_secs_f64()) as u64
            } else {
                0
            },
            ..SearchStats::default()
        };

        (best_move, stats)
    }
}