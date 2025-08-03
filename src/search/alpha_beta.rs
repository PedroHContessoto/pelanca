use crate::core::*;
use super::evaluation::*;
use std::time::{Duration, Instant};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Resultado da busca Alpha-Beta
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u8,
    pub nodes_searched: u64,
    pub time_elapsed: Duration,
}

/// Motor de busca Alpha-Beta
pub struct AlphaBetaEngine {
    pub nodes_searched: AtomicU64,
    pub start_time: Option<Instant>,
    pub max_time: Option<Duration>,
    pub should_stop: AtomicBool,
    pub threads: usize,
}

impl AlphaBetaEngine {
    pub fn new() -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: num_cpus::get().max(1),
        }
    }
    
    pub fn new_with_threads(threads: usize) -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: threads.max(1),
        }
    }
    
    /// Busca o melhor movimento com limite de profundidade
    pub fn search(&mut self, board: &Board, depth: u8) -> SearchResult {
        self.nodes_searched.store(0, Ordering::Relaxed);
        self.should_stop.store(false, Ordering::Relaxed);
        self.start_time = Some(Instant::now());
        
        let (best_move, score) = self.alpha_beta_root_parallel(board, depth);
        
        SearchResult {
            best_move,
            score,
            depth,
            nodes_searched: self.nodes_searched.load(Ordering::Relaxed),
            time_elapsed: self.start_time.unwrap().elapsed(),
        }
    }
    
    /// Busca com limite de tempo
    pub fn search_time(&mut self, board: &Board, max_time: Duration, max_depth: u8) -> SearchResult {
        self.nodes_searched.store(0, Ordering::Relaxed);
        self.should_stop.store(false, Ordering::Relaxed);
        self.start_time = Some(Instant::now());
        self.max_time = Some(max_time);
        
        let mut best_result = SearchResult {
            best_move: None,
            score: 0,
            depth: 0,
            nodes_searched: 0,
            time_elapsed: Duration::from_millis(0),
        };
        
        // Busca iterativa por profundidade
        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }
            
            let (best_move, score) = self.alpha_beta_root_parallel(board, depth);
            
            best_result = SearchResult {
                best_move,
                score,
                depth,
                nodes_searched: self.nodes_searched.load(Ordering::Relaxed),
                time_elapsed: self.start_time.unwrap().elapsed(),
            };
            
            // Se encontrou mate, para a busca
            if score.abs() > 29000 {
                break;
            }
        }
        
        self.max_time = None;
        best_result
    }
    
    /// Busca Alpha-Beta na raiz (paralela)
    fn alpha_beta_root_parallel(&self, board: &Board, depth: u8) -> (Option<Move>, i32) {
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board));
        }
        
        if depth <= 2 {
            // Use versão sequencial para profundidades baixas
            return self.alpha_beta_root_sequential(board, depth);
        }
        
        // Paraleliza apenas no primeiro nível, igual ao perft_parallel
        let results: Vec<(Move, i32)> = moves.par_iter().filter_map(|&mv| {
            if self.should_stop() {
                return None;
            }
            
            let mut board_clone = *board; // Copy barato devido ao trait Copy
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let score = -self.alpha_beta_sequential(&board_clone, depth - 1, i32::MIN, i32::MAX, false);
                board_clone.unmake_move(mv, undo_info); // Cleanup (opcional, pois board_clone será descartado)
                Some((mv, score))
            } else {
                board_clone.unmake_move(mv, undo_info);
                None
            }
        }).collect();
        
        // Encontra o melhor movimento
        if let Some((best_move, best_score)) = results.into_iter().max_by_key(|(_, score)| *score) {
            (Some(best_move), best_score)
        } else {
            (None, i32::MIN)
        }
    }
    
    /// Busca Alpha-Beta na raiz (sequencial)
    fn alpha_beta_root_sequential(&self, board: &Board, depth: u8) -> (Option<Move>, i32) {
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board));
        }
        
        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut alpha = i32::MIN;
        let beta = i32::MAX;
        
        for mv in moves {
            if self.should_stop() {
                break;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let score = -self.alpha_beta_sequential(&board_clone, depth - 1, -beta, -alpha, false);
                
                if score > best_score {
                    best_score = score;
                    best_move = Some(mv);
                }
                
                alpha = alpha.max(score);
            }
            
            board_clone.unmake_move(mv, undo_info);
        }
        
        (best_move, best_score)
    }
    
    
    /// Algoritmo Alpha-Beta sequencial (igual ao perft_with_tt)
    fn alpha_beta_sequential(&self, board: &Board, depth: u8, mut alpha: i32, mut beta: i32, is_maximizing: bool) -> i32 {
        self.nodes_searched.fetch_add(1, Ordering::Relaxed);
        
        // Verifica tempo limite
        if self.should_stop() {
            return 0;
        }
        
        // Verifica posição terminal
        if let Some(terminal_score) = is_terminal_position(board) {
            return if is_maximizing { terminal_score } else { -terminal_score };
        }
        
        // Se chegou na profundidade limite, avalia posição
        if depth == 0 {
            return if is_maximizing {
                evaluate_position(board)
            } else {
                -evaluate_position(board)
            };
        }
        
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            // Não há movimentos legais (mate ou afogamento)
            if board.is_king_in_check(board.to_move) {
                // Xeque-mate
                return if is_maximizing { -30000 + depth as i32 } else { 30000 - depth as i32 };
            } else {
                // Afogamento
                return 0;
            }
        }
        
        if is_maximizing {
            let mut max_eval = i32::MIN;
            
            for mv in moves {
                if self.should_stop() {
                    break;
                }
                
                let mut board_clone = *board;
                let undo_info = board_clone.make_move_with_undo(mv);
                let previous_to_move = !board_clone.to_move;
                
                if !board_clone.is_king_in_check(previous_to_move) {
                    let eval = self.alpha_beta_sequential(&board_clone, depth - 1, alpha, beta, false);
                    max_eval = max_eval.max(eval);
                    alpha = alpha.max(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(mv, undo_info);
            }
            
            max_eval
        } else {
            let mut min_eval = i32::MAX;
            
            for mv in moves {
                if self.should_stop() {
                    break;
                }
                
                let mut board_clone = *board;
                let undo_info = board_clone.make_move_with_undo(mv);
                let previous_to_move = !board_clone.to_move;
                
                if !board_clone.is_king_in_check(previous_to_move) {
                    let eval = self.alpha_beta_sequential(&board_clone, depth - 1, alpha, beta, true);
                    min_eval = min_eval.min(eval);
                    beta = beta.min(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(mv, undo_info);
            }
            
            min_eval
        }
    }
    
    /// Verifica se deve parar a busca (por tempo)
    fn should_stop(&self) -> bool {
        if let (Some(start), Some(max_time)) = (self.start_time, self.max_time) {
            start.elapsed() >= max_time
        } else {
            false
        }
    }
}

impl Default for AlphaBetaEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Função de conveniência para busca rápida
pub fn find_best_move(board: &Board, depth: u8) -> SearchResult {
    let mut engine = AlphaBetaEngine::new();
    engine.search(board, depth)
}

/// Função de conveniência para busca com limite de tempo
pub fn find_best_move_time(board: &Board, time_limit: Duration, max_depth: u8) -> SearchResult {
    let mut engine = AlphaBetaEngine::new();
    engine.search_time(board, time_limit, max_depth)
}