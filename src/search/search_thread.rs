// Sistema de busca paralela com múltiplas threads

use super::*;
use crate::core::*;
use std::sync::{Arc, Mutex};
use std::thread;
use rayon::prelude::*;

/// Thread de busca individual
pub struct SearchThread {
    id: usize,
    board: Board,
    controller: Arc<SearchController>,
    local_stats: SearchStats,
}

impl SearchThread {
    pub fn new(id: usize, board: Board, controller: Arc<SearchController>) -> Self {
        SearchThread {
            id,
            board,
            controller,
            local_stats: SearchStats::new(),
        }
    }

    /// Executa busca nesta thread
    pub fn search(&mut self, depth: u8, alpha: i32, beta: i32) -> (Move, i32) {
        // Cada thread pode ter variação na busca para diversificar
        let depth_variation = if self.id > 0 && depth > 6 {
            (self.id % 2) as u8 // Threads ímpares buscam 1 ply a menos
        } else {
            0
        };
        let actual_depth = depth.saturating_sub(depth_variation);
        // Busca com pequena variação no aspiration window
        let window_variation = (self.id * 10) as i32;
        let adjusted_alpha = alpha - window_variation;
        let adjusted_beta = beta + window_variation;
        let (mv, score, _) = super::search_root(
            &mut self.board,
            actual_depth,
            adjusted_alpha,
            adjusted_beta,
            &self.controller
        );
        (mv, score)
    }
}

/// Pool de threads para busca paralela
pub struct ThreadPool {
    threads: Vec<Arc<Mutex<SearchThread>>>,
    main_board: Board,
}

impl ThreadPool {
    pub fn new(board: Board, num_threads: usize, controller: Arc<SearchController>) -> Self {
        let threads = (0..num_threads)
            .map(|id| {
                Arc::new(Mutex::new(SearchThread::new(
                    id,
                    board,
                    controller.clone()
                )))
            })
            .collect();

        ThreadPool {
            threads,
            main_board: board,
        }
    }

    /// Busca paralela com múltiplas threads
    pub fn search_parallel(&self, depth: u8, alpha: i32, beta: i32) -> (Move, i32, Vec<Move>) {
        let num_threads = self.threads.len();

        // Caso especial: apenas 1 thread
        if num_threads == 1 {
            let mut thread = self.threads[0].lock().unwrap();
            let (mv, score) = thread.search(depth, alpha, beta);
            return (mv, score, vec![mv]);
        }

        // Coleta resultados de todas as threads
        let results: Vec<(Move, i32)> = self.threads
            .par_iter()
            .map(|thread_mutex| {
                let mut thread = thread_mutex.lock().unwrap();
                thread.search(depth, alpha, beta)
            })
            .collect();

        // Encontra melhor resultado
        let (best_move, best_score) = results
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .unwrap_or((Move { from: 0, to: 0, promotion: None, is_castling: false, is_en_passant: false }, -INF));

        // TODO: Extrair PV completa
        (best_move, best_score, vec![best_move])
    }
}

/// Lazy SMP (Symmetric MultiProcessing) - implementação moderna
pub struct LazySMP {
    controller: Arc<SearchController>,
    threads: Vec<thread::JoinHandle<SearchResult>>,
}

struct SearchResult {
    thread_id: usize,
    best_move: Move,
    score: i32,
    pv: Vec<Move>,
    nodes: u64,
}

impl LazySMP {
    /// Inicia busca Lazy SMP com múltiplas threads
    pub fn search(
        board: &Board,
        depth: u8,
        num_threads: usize,
        controller: Arc<SearchController>,
    ) -> (Move, i32, Vec<Move>) {
        // Canal para coletar resultados
        let (tx, rx) = std::sync::mpsc::channel();

        // Inicia threads de busca
        let threads: Vec<_> = (0..num_threads)
            .map(|id| {
                let board_clone = *board;
                let controller_clone = controller.clone();
                let tx_clone = tx.clone();
                thread::spawn(move || {
                    let mut local_board = board_clone;
                    // Variação de profundidade para diversificar busca
                    let depth_offset = if id == 0 {
                        0 // Thread principal busca profundidade completa
                    } else {
                        ((id - 1) % 3) as u8 // Outras threads variam 0-2 plys
                    };
                    let search_depth = depth.saturating_sub(depth_offset);
                    // Busca com aspiration windows variados
                    let window = 50 + (id * 25) as i32;
                    let (mv, score, pv) = super::search_root(
                        &mut local_board,
                        search_depth,
                        -INF + window,
                        INF - window,
                        &controller_clone,
                    );
                    let result = SearchResult {
                        thread_id: id,
                        best_move: mv,
                        score,
                        pv,
                        nodes: controller_clone.node_counter.load(std::sync::atomic::Ordering::Relaxed),
                    };
                    let _ = tx_clone.send(result);
                })
            })
            .collect();

        drop(tx); // Fecha canal de envio

        // Coleta resultados conforme chegam
        let mut best_result: Option<SearchResult> = None;
        let mut results_count = 0;
        for result in rx {
            let score = result.score;
            let thread_id = result.thread_id;
            results_count += 1;
            // Atualiza melhor resultado
            if best_result.is_none() || score > best_result.as_ref().unwrap().score {
                best_result = Some(result);
            }
            // Para busca se encontrou mate
            if score.abs() >= MATE_THRESHOLD {
                controller.stop();
                break;
            }
            // Pode parar após receber resultado da thread principal se for bom
            if thread_id == 0 && results_count >= num_threads / 2 {
                controller.stop();
            }
        }

        // Aguarda threads terminarem
        for handle in threads {
            let _ = handle.join();
        }

        // Retorna melhor resultado
        if let Some(result) = best_result {
            (result.best_move, result.score, result.pv)
        } else {
            // Fallback
            let moves = board.generate_all_moves();
            (moves[0], 0, vec![moves[0]])
        }
    }
}