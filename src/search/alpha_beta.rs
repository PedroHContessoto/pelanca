use crate::core::*;
use super::evaluation::*;
use super::move_ordering::*;
use super::tactical_analysis::*;
use super::lmr::*;
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
    pub pv_line: Vec<Move>, // Linha principal completa
}

/// Motor de busca Alpha-Beta
pub struct AlphaBetaEngine {
    pub nodes_searched: AtomicU64,
    pub start_time: Option<Instant>,
    pub max_time: Option<Duration>,
    pub should_stop: AtomicBool,
    pub threads: usize,
    pub lmr_config: LMRConfig,
}

impl AlphaBetaEngine {
    pub fn new() -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: num_cpus::get().max(1),
            lmr_config: LMRConfig::ultra_aggressive(), // Config agressiva para depth 17
        }
    }
    
    pub fn new_with_threads(threads: usize) -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: threads.max(1),
            lmr_config: LMRConfig::ultra_aggressive(), // Config agressiva para depth 17
        }
    }
    
    
    /// Busca com limite de tempo (com linha de pensamento completa)
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
            pv_line: Vec::new(),
        };
        
        // Busca iterativa por profundidade
        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }
            
            let depth_start_time = Instant::now();
            let (best_move, score, pv_line) = self.alpha_beta_root_parallel(board, depth);
            let depth_time = depth_start_time.elapsed();
            
            best_result = SearchResult {
                best_move,
                score,
                depth,
                nodes_searched: self.nodes_searched.load(Ordering::Relaxed),
                time_elapsed: self.start_time.unwrap().elapsed(),
                pv_line: pv_line.clone(),
            };
            
            // Imprime linha de pensamento para esta profundidade
            self.print_thinking_line(&best_result);
            
            // Se encontrou mate, para a busca
            if score.abs() > 29000 {
                break;
            }
        }
        
        self.max_time = None;
        best_result
    }
    
    /// Busca o melhor movimento com limite de profundidade (com linha de pensamento)
    pub fn search(&mut self, board: &Board, depth: u8) -> SearchResult {
        self.nodes_searched.store(0, Ordering::Relaxed);
        self.should_stop.store(false, Ordering::Relaxed);
        self.start_time = Some(Instant::now());
        
        // SEMPRE usa busca iterativa para profundidades > 1 (máxima inteligência)
        if depth > 1 {
            return self.search_time(board, Duration::from_secs(3600), depth); // 1 hora como limite máximo
        }
        
        let (best_move, score, pv_line) = self.alpha_beta_root_parallel(board, depth);
        
        let result = SearchResult {
            best_move,
            score,
            depth,
            nodes_searched: self.nodes_searched.load(Ordering::Relaxed),
            time_elapsed: self.start_time.unwrap().elapsed(),
            pv_line,
        };
        
        // Imprime linha de pensamento
        self.print_thinking_line(&result);
        
        result
    }
    
    /// Imprime linha de pensamento UCI com PV completa
    fn print_thinking_line(&self, result: &SearchResult) {
        let nps = if result.time_elapsed.as_millis() > 0 {
            (result.nodes_searched as f64 / result.time_elapsed.as_secs_f64()) as u64
        } else {
            0
        };
        let time_ms = result.time_elapsed.as_millis();
        
        // Formata score (centipawns ou mate)
        let score_output = if result.score.abs() > 29000 {
            let mate_in = (30000 - result.score.abs()) / 2 + 1;
            if result.score > 0 {
                format!("score mate {}", mate_in)
            } else {
                format!("score mate -{}", mate_in)
            }
        } else {
            format!("score cp {}", result.score)
        };
        
        // Constrói PV line completa
        let pv = if result.pv_line.is_empty() {
            "pv".to_string()
        } else {
            let pv_moves: Vec<String> = result.pv_line.iter()
                .map(|mv| self.format_uci_move(*mv))
                .collect();
            format!("pv {}", pv_moves.join(" "))
        };
        
        println!("info depth {} {} nodes {} nps {} time {} hashfull {} tbhits {} multipv {} {}", 
                 result.depth, 
                 score_output, 
                 result.nodes_searched, 
                 nps, 
                 time_ms,
                 0, // hashfull
                 0, // tbhits
                 1, // multipv
                 pv // linha principal completa
        );
    }
    
    /// Formata movimento para UCI
    fn format_uci_move(&self, mv: Move) -> String {
        let from_file = (mv.from % 8) as u8 + b'a';
        let from_rank = (mv.from / 8) as u8 + b'1';
        let to_file = (mv.to % 8) as u8 + b'a';
        let to_rank = (mv.to / 8) as u8 + b'1';
        
        let mut result = format!("{}{}{}{}", 
                                from_file as char, 
                                from_rank as char,
                                to_file as char, 
                                to_rank as char);
        
        if let Some(promotion) = mv.promotion {
            result.push(match promotion {
                PieceKind::Queen => 'q',
                PieceKind::Rook => 'r',
                PieceKind::Bishop => 'b',
                PieceKind::Knight => 'n',
                _ => 'q',
            });
        }
        
        result
    }
    
    /// Busca Alpha-Beta na raiz (paralela com ordenação inteligente)
    fn alpha_beta_root_parallel(&self, board: &Board, depth: u8) -> (Option<Move>, i32, Vec<Move>) {
        let mut moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board), Vec::new());
        }
        
        // FILTRO ADAPTATIVO para eliminar movimentos não promissores
        MoveFilter::filter_unpromising_moves(board, &mut moves);
        
        // ORDENAÇÃO INTELIGENTE para máxima performance
        order_moves(board, &mut moves);
        
        if depth <= 1 {
            // Use versão sequencial apenas para depth 1
            return self.alpha_beta_root_sequential(board, depth, &moves);
        }
        
        // PARALELIZAÇÃO MASSIVA - cada movimento em thread separada
        let results: Vec<(Move, i32, Vec<Move>)> = moves.par_iter().filter_map(|&mv| {
            if self.should_stop() {
                return None;
            }
            
            let mut board_clone = *board; // Copy barato devido ao trait Copy
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                // Busca com janela estreita para economizar nós
                let (score, mut pv_line) = self.alpha_beta_with_pv(&board_clone, depth - 1, i32::MIN, i32::MAX, false);
                
                // Adiciona movimento atual no início da PV
                pv_line.insert(0, mv);
                
                board_clone.unmake_move(mv, undo_info);
                Some((mv, -score, pv_line))
            } else {
                board_clone.unmake_move(mv, undo_info);
                None
            }
        }).collect();
        
        // Encontra o melhor movimento com PV
        if let Some((best_move, best_score, pv_line)) = results.into_iter().max_by_key(|(_, score, _)| *score) {
            (Some(best_move), best_score, pv_line)
        } else {
            (None, i32::MIN, Vec::new())
        }
    }
    
    /// Busca Alpha-Beta na raiz (sequencial com ordenação)
    fn alpha_beta_root_sequential(&self, board: &Board, depth: u8, moves: &[Move]) -> (Option<Move>, i32, Vec<Move>) {
        if moves.is_empty() {
            return (None, evaluate_position(board), Vec::new());
        }
        
        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut best_pv = Vec::new();
        let mut alpha = i32::MIN;
        let beta = i32::MAX;
        
        for &mv in moves {
            if self.should_stop() {
                break;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let (score, mut pv_line) = self.alpha_beta_with_pv(&board_clone, depth - 1, -beta, -alpha, false);
                let score = -score;
                
                if score > best_score {
                    best_score = score;
                    best_move = Some(mv);
                    
                    // Constrói PV completa
                    best_pv = vec![mv];
                    best_pv.extend(pv_line);
                }
                
                alpha = alpha.max(score);
                
                // Alpha-Beta cutoff
                if beta <= alpha {
                    board_clone.unmake_move(mv, undo_info);
                    break;
                }
            }
            
            board_clone.unmake_move(mv, undo_info);
        }
        
        (best_move, best_score, best_pv)
    }
    
    
    /// Algoritmo Alpha-Beta sequencial com ordenação inteligente
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
        
        // Se chegou na profundidade limite, usa quiescence search para táticas
        if depth == 0 {
            return self.quiescence_search(board, alpha, beta, is_maximizing, 3); // Reduzido para 3
        }
        
        let mut moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            // Não há movimentos legais (mate ou afogamento)
            if board.is_king_in_check(board.to_move) {
                // Xeque-mate - mais próximo é melhor
                return if is_maximizing { -30000 + depth as i32 } else { 30000 - depth as i32 };
            } else {
                // Afogamento
                return 0;
            }
        }
        
        // FILTRO ADAPTATIVO para controlar explosão combinatória
        MoveFilter::filter_unpromising_moves(board, &mut moves);
        
        // ORDENAÇÃO INTELIGENTE para máximas podas
        order_moves(board, &mut moves);
        
        if is_maximizing {
            let mut max_eval = i32::MIN;
            
            for (move_index, mv) in moves.iter().enumerate() {
                if self.should_stop() {
                    break;
                }
                
                let mut board_clone = *board;
                let undo_info = board_clone.make_move_with_undo(*mv);
                let previous_to_move = !board_clone.to_move;
                
                if !board_clone.is_king_in_check(previous_to_move) {
                    // LATE MOVE REDUCTION (LMR) usando configuração agressiva
                    let is_tactical = LateMovePruner::is_tactical_move(board, *mv);
                    let reduction = self.lmr_config.calculate_reduction(move_index, depth, is_tactical);
                    let search_depth = depth.saturating_sub(1 + reduction);
                    
                    let eval = self.alpha_beta_sequential(&board_clone, search_depth, alpha, beta, false);
                    max_eval = max_eval.max(eval);
                    alpha = alpha.max(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(*mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(*mv, undo_info);
            }
            
            max_eval
        } else {
            let mut min_eval = i32::MAX;
            
            for (move_index, mv) in moves.iter().enumerate() {
                if self.should_stop() {
                    break;
                }
                
                let mut board_clone = *board;
                let undo_info = board_clone.make_move_with_undo(*mv);
                let previous_to_move = !board_clone.to_move;
                
                if !board_clone.is_king_in_check(previous_to_move) {
                    // LATE MOVE REDUCTION (LMR) usando configuração agressiva
                    let is_tactical = LateMovePruner::is_tactical_move(board, *mv);
                    let reduction = self.lmr_config.calculate_reduction(move_index, depth, is_tactical);
                    let search_depth = depth.saturating_sub(1 + reduction);
                    
                    let eval = self.alpha_beta_sequential(&board_clone, search_depth, alpha, beta, true);
                    min_eval = min_eval.min(eval);
                    beta = beta.min(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(*mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(*mv, undo_info);
            }
            
            min_eval
        }
    }
    
    /// Alpha-Beta com Quiescence Search integrada
    fn alpha_beta_with_quiescence(&self, board: &Board, depth: u8, mut alpha: i32, mut beta: i32, is_maximizing: bool) -> i32 {
        if depth == 0 {
            return self.quiescence_search(board, alpha, beta, is_maximizing, 6);
        }
        
        self.alpha_beta_sequential(board, depth, alpha, beta, is_maximizing)
    }
    
    /// Quiescence Search - busca apenas capturas e xeques para evitar horizon effect
    fn quiescence_search(&self, board: &Board, mut alpha: i32, mut beta: i32, is_maximizing: bool, qs_depth: u8) -> i32 {
        self.nodes_searched.fetch_add(1, Ordering::Relaxed);
        
        if self.should_stop() || qs_depth == 0 {
            return if is_maximizing {
                evaluate_position(board)
            } else {
                -evaluate_position(board)
            };
        }
        
        // Stand-pat: avaliação estática
        let stand_pat = if is_maximizing {
            evaluate_position(board)
        } else {
            -evaluate_position(board)
        };
        
        if is_maximizing {
            alpha = alpha.max(stand_pat);
            if beta <= alpha {
                return alpha; // Beta cutoff
            }
        } else {
            beta = beta.min(stand_pat);
            if beta <= alpha {
                return beta; // Alpha cutoff  
            }
        }
        
        // Gera apenas capturas e xeques (movimentos "ruidosos")
        let tactical_moves = generate_tactical_moves(board);
        
        for mv in tactical_moves {
            if self.should_stop() {
                break;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let score = self.quiescence_search(&board_clone, alpha, beta, !is_maximizing, qs_depth - 1);
                
                if is_maximizing {
                    alpha = alpha.max(score);
                    if beta <= alpha {
                        board_clone.unmake_move(mv, undo_info);
                        break;
                    }
                } else {
                    beta = beta.min(score);
                    if beta <= alpha {
                        board_clone.unmake_move(mv, undo_info);
                        break;
                    }
                }
            }
            
            board_clone.unmake_move(mv, undo_info);
        }
        
        if is_maximizing { alpha } else { beta }
    }
    
    /// Alpha-Beta que retorna score e PV line
    fn alpha_beta_with_pv(&self, board: &Board, depth: u8, mut alpha: i32, mut beta: i32, is_maximizing: bool) -> (i32, Vec<Move>) {
        self.nodes_searched.fetch_add(1, Ordering::Relaxed);
        
        if self.should_stop() {
            return (0, Vec::new());
        }
        
        // Verifica posição terminal
        if let Some(terminal_score) = is_terminal_position(board) {
            let score = if is_maximizing { terminal_score } else { -terminal_score };
            return (score, Vec::new());
        }
        
        // Se chegou na profundidade limite, usa quiescence search
        if depth == 0 {
            let score = self.quiescence_search(board, alpha, beta, is_maximizing, 3); // Reduzido para 3
            return (score, Vec::new());
        }
        
        let mut moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            // Não há movimentos legais (mate ou afogamento)
            if board.is_king_in_check(board.to_move) {
                let score = if is_maximizing { -30000 + depth as i32 } else { 30000 - depth as i32 };
                return (score, Vec::new());
            } else {
                return (0, Vec::new()); // Afogamento
            }
        }
        
        // FILTRO ADAPTATIVO para controlar explosão combinatória
        MoveFilter::filter_unpromising_moves(board, &mut moves);
        
        // ORDENAÇÃO INTELIGENTE para máximas podas
        order_moves(board, &mut moves);
        
        let mut best_pv = Vec::new();
        
        if is_maximizing {
            let mut max_eval = i32::MIN;
            
            for (move_index, mv) in moves.iter().enumerate() {
                if self.should_stop() {
                    break;
                }
                
                let mut board_clone = *board;
                let undo_info = board_clone.make_move_with_undo(*mv);
                let previous_to_move = !board_clone.to_move;
                
                if !board_clone.is_king_in_check(previous_to_move) {
                    // LMR para alpha_beta_with_pv também
                    let is_tactical = LateMovePruner::is_tactical_move(board, *mv);
                    let reduction = self.lmr_config.calculate_reduction(move_index, depth, is_tactical);
                    let search_depth = depth.saturating_sub(1 + reduction);
                    
                    let (eval, mut pv_line) = self.alpha_beta_with_pv(&board_clone, search_depth, alpha, beta, false);
                    
                    if eval > max_eval {
                        max_eval = eval;
                        
                        // Atualiza melhor PV
                        best_pv = vec![*mv];
                        best_pv.extend(pv_line);
                    }
                    
                    alpha = alpha.max(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(*mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(*mv, undo_info);
            }
            
            (max_eval, best_pv)
        } else {
            let mut min_eval = i32::MAX;
            
            for (move_index, mv) in moves.iter().enumerate() {
                if self.should_stop() {
                    break;
                }
                
                let mut board_clone = *board;
                let undo_info = board_clone.make_move_with_undo(*mv);
                let previous_to_move = !board_clone.to_move;
                
                if !board_clone.is_king_in_check(previous_to_move) {
                    // LMR para alpha_beta_with_pv também
                    let is_tactical = LateMovePruner::is_tactical_move(board, *mv);
                    let reduction = self.lmr_config.calculate_reduction(move_index, depth, is_tactical);
                    let search_depth = depth.saturating_sub(1 + reduction);
                    
                    let (eval, mut pv_line) = self.alpha_beta_with_pv(&board_clone, search_depth, alpha, beta, true);
                    
                    if eval < min_eval {
                        min_eval = eval;
                        
                        // Atualiza melhor PV
                        best_pv = vec![*mv];
                        best_pv.extend(pv_line);
                    }
                    
                    beta = beta.min(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(*mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(*mv, undo_info);
            }
            
            (min_eval, best_pv)
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

/// Gera apenas movimentos táticos (capturas, promoções, xeques) para quiescence search
fn generate_tactical_moves(board: &Board) -> Vec<Move> {
    let all_moves = board.generate_legal_moves();
    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };
    
    let mut tactical_moves = Vec::with_capacity(all_moves.len() / 2);
    
    for mv in all_moves {
        let to_bb = 1u64 << mv.to;
        
        // Capturas
        if (enemy_pieces & to_bb) != 0 {
            tactical_moves.push(mv);
            continue;
        }
        
        // Promoções
        if mv.promotion.is_some() {
            tactical_moves.push(mv);
            continue;
        }
        
        // En passant
        if mv.is_en_passant {
            tactical_moves.push(mv);
            continue;
        }
        
        // Xeques (verifica se o movimento dá xeque)
        let mut test_board = *board;
        if test_board.make_move(mv) {
            let enemy_color = !board.to_move;
            if test_board.is_king_in_check(enemy_color) {
                tactical_moves.push(mv);
            }
        }
    }
    
    // Ordena movimentos táticos por valor
    order_moves(board, &mut tactical_moves);
    tactical_moves
}