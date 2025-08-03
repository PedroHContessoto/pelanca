use crate::core::*;
use crate::engine::{TranspositionTable, TT_EXACT, TT_ALPHA, TT_BETA};
use crate::search::{order_moves_advanced, KillerMoves, HistoryTable, quiescence_search};
use std::time::{Duration, Instant};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Resultado da busca Alpha-Beta
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u8,
    pub nodes_searched: u64,
    pub time_elapsed: Duration,
    pub pv_line: Vec<Move>,
}

/// Motor Alpha-Beta com TT compartilhada e otimizações avançadas
pub struct AlphaBetaTTEngine {
    pub nodes_searched: AtomicU64,
    pub start_time: Option<Instant>,
    pub max_time: Option<Duration>,
    pub should_stop: AtomicBool,
    pub threads: usize,
    pub tt: Arc<Mutex<TranspositionTable>>, // TT compartilhada
    pub killers: KillerMoves,
    pub history: HistoryTable,
}

impl AlphaBetaTTEngine {
    pub fn new() -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: num_cpus::get().max(1),
            tt: Arc::new(Mutex::new(TranspositionTable::new())),
            killers: KillerMoves::new(),
            history: HistoryTable::new(),
        }
    }
    
    pub fn new_with_threads(threads: usize) -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: threads.max(1),
            tt: Arc::new(Mutex::new(TranspositionTable::new())),
            killers: KillerMoves::new(),
            history: HistoryTable::new(),
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
            pv_line: Vec::new(),
        };
        
        // Busca iterativa por profundidade
        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }
            
            let (best_move, score, pv_line) = self.alpha_beta_root_parallel(board, depth);
            
            best_result = SearchResult {
                best_move,
                score,
                depth,
                nodes_searched: self.nodes_searched.load(Ordering::Relaxed),
                time_elapsed: self.start_time.unwrap().elapsed(),
                pv_line: pv_line.clone(),
            };
            
            // Imprime linha de pensamento
            self.print_thinking_line(&best_result);
            
            // Se encontrou mate, para a busca
            if score.abs() > 29000 {
                break;
            }
        }
        
        self.max_time = None;
        best_result
    }
    
    /// Busca com limite de profundidade
    pub fn search(&mut self, board: &Board, depth: u8) -> SearchResult {
        self.nodes_searched.store(0, Ordering::Relaxed);
        self.should_stop.store(false, Ordering::Relaxed);
        self.start_time = Some(Instant::now());
        
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
    
    /// Alpha-Beta paralelo na raiz (baseado em perft_parallel)
    fn alpha_beta_root_parallel(&self, board: &Board, depth: u8) -> (Option<Move>, i32, Vec<Move>) {
        if depth <= 2 {
            // Use versão sequencial para profundidades baixas (como perft_parallel)
            return self.alpha_beta_root_sequential(board, depth);
        }
        
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board), Vec::new());
        }
        
        // Paralelização massiva - cada movimento em thread separada (igual perft_parallel)
        let results: Vec<(Move, i32, Vec<Move>)> = moves.par_iter().filter_map(|&mv| {
            if self.should_stop() {
                return None;
            }
            
            let mut board_clone = *board; // Copy barato devido ao trait Copy
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                // Cada thread tem sua própria TT
                let (score, mut pv_line) = self.alpha_beta_with_tt(&board_clone, depth - 1, i32::MIN, i32::MAX, false, &mut TranspositionTable::new());
                
                // Adiciona movimento atual no início da PV
                pv_line.insert(0, mv);
                
                board_clone.unmake_move(mv, undo_info);
                Some((mv, -score, pv_line))
            } else {
                board_clone.unmake_move(mv, undo_info);
                None
            }
        }).collect();
        
        // Encontra o melhor movimento
        if let Some((best_move, best_score, pv_line)) = results.into_iter().max_by_key(|(_, score, _)| *score) {
            (Some(best_move), best_score, pv_line)
        } else {
            // Fallback
            (Some(moves[0]), 0, vec![moves[0]])
        }
    }
    
    /// Alpha-Beta sequencial na raiz 
    fn alpha_beta_root_sequential(&self, board: &Board, depth: u8) -> (Option<Move>, i32, Vec<Move>) {
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board), Vec::new());
        }
        
        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut best_pv = Vec::new();
        let mut alpha = i32::MIN;
        let beta = i32::MAX;
        let mut tt = TranspositionTable::new();
        
        for &mv in &moves {
            if self.should_stop() {
                break;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let (score, mut pv_line) = self.alpha_beta_with_tt(&board_clone, depth - 1, -beta, -alpha, false, &mut tt);
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
    
    /// Alpha-Beta com TT
    fn alpha_beta_with_tt(&self, board: &Board, depth: u8, mut alpha: i32, mut beta: i32, is_maximizing: bool, tt: &mut TranspositionTable) -> (i32, Vec<Move>) {
        self.nodes_searched.fetch_add(1, Ordering::Relaxed);
        
        if self.should_stop() {
            return (0, Vec::new());
        }
        
        if depth == 0 {
            // Usa busca quiescente ao invés de avaliação direta
            let mut tt_lock = self.tt.lock().unwrap();
            let score = if is_maximizing {
                quiescence_search(board, alpha, beta, 6, &self.nodes_searched, &mut *tt_lock, evaluate_position)
            } else {
                -quiescence_search(board, -beta, -alpha, 6, &self.nodes_searched, &mut *tt_lock, evaluate_position)
            };
            return (score, Vec::new());
        }
        
        // Verifica cache primeiro
        if let Some(cached_score) = tt.probe(board.zobrist_hash, depth, alpha, beta) {
            return (cached_score, Vec::new()); // Cache hit
        }
        
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            // Não há movimentos legais (mate ou afogamento)
            let score = if board.is_king_in_check(board.to_move) {
                // Xeque-mate - mais próximo é melhor
                if is_maximizing { -30000 + depth as i32 } else { 30000 - depth as i32 }
            } else {
                // Afogamento
                0
            };
            
            // Cache resultado
            tt.store(board.zobrist_hash, depth, score, TT_EXACT, None);
            return (score, Vec::new());
        }
        
        let mut best_pv = Vec::new();
        
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
                    let (eval, mut pv_line) = self.alpha_beta_with_tt(&board_clone, depth - 1, alpha, beta, false, tt);
                    
                    if eval > max_eval {
                        max_eval = eval;
                        
                        // Atualiza melhor PV
                        best_pv = vec![mv];
                        best_pv.extend(pv_line);
                    }
                    
                    alpha = alpha.max(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(mv, undo_info);
            }
            
            // Cache resultado com flag apropriada
            let flag = if max_eval <= alpha {
                TT_ALPHA
            } else if max_eval >= beta {
                TT_BETA
            } else {
                TT_EXACT
            };
            tt.store(board.zobrist_hash, depth, max_eval, flag, best_pv.first().copied());
            (max_eval, best_pv)
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
                    let (eval, mut pv_line) = self.alpha_beta_with_tt(&board_clone, depth - 1, alpha, beta, true, tt);
                    
                    if eval < min_eval {
                        min_eval = eval;
                        
                        // Atualiza melhor PV
                        best_pv = vec![mv];
                        best_pv.extend(pv_line);
                    }
                    
                    beta = beta.min(eval);
                    
                    // Poda Alpha-Beta
                    if beta <= alpha {
                        board_clone.unmake_move(mv, undo_info);
                        break;
                    }
                }
                
                board_clone.unmake_move(mv, undo_info);
            }
            
            // Cache resultado com flag apropriada
            let flag = if min_eval <= alpha {
                TT_ALPHA
            } else if min_eval >= beta {
                TT_BETA
            } else {
                TT_EXACT
            };
            tt.store(board.zobrist_hash, depth, min_eval, flag, best_pv.first().copied());
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
    
    /// Imprime linha de pensamento UCI
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
        
        // Constrói PV line
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
                 0, // hashfull (seria melhor com TT compartilhada)
                 0, // tbhits
                 1, // multipv
                 pv
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
}

impl Default for AlphaBetaTTEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Função de conveniência para busca rápida
pub fn find_best_move_tt(board: &Board, depth: u8) -> SearchResult {
    let mut engine = AlphaBetaTTEngine::new();
    engine.search(board, depth)
}

/// Função de conveniência para busca com limite de tempo
pub fn find_best_move_time_tt(board: &Board, time_limit: Duration, max_depth: u8) -> SearchResult {
    let mut engine = AlphaBetaTTEngine::new();
    engine.search_time(board, time_limit, max_depth)
}

/// Avaliação simples de posição (material + posição básica)
fn evaluate_position(board: &Board) -> i32 {
    use crate::utils::*;
    
    // Conta material
    let white_pawns = popcount(board.white_pieces & board.pawns) as i32;
    let black_pawns = popcount(board.black_pieces & board.pawns) as i32;
    let white_knights = popcount(board.white_pieces & board.knights) as i32;
    let black_knights = popcount(board.black_pieces & board.knights) as i32;
    let white_bishops = popcount(board.white_pieces & board.bishops) as i32;
    let black_bishops = popcount(board.black_pieces & board.bishops) as i32;
    let white_rooks = popcount(board.white_pieces & board.rooks) as i32;
    let black_rooks = popcount(board.black_pieces & board.rooks) as i32;
    let white_queens = popcount(board.white_pieces & board.queens) as i32;
    let black_queens = popcount(board.black_pieces & board.queens) as i32;
    
    // Material
    let material = (white_pawns - black_pawns) * 100 +
                  (white_knights - black_knights) * 320 +
                  (white_bishops - black_bishops) * 330 +
                  (white_rooks - black_rooks) * 500 +
                  (white_queens - black_queens) * 900;
    
    // Bônus por controle do centro
    const CENTER: u64 = 0x0000001818000000; // e4, e5, d4, d5
    let white_center = popcount(board.white_pieces & CENTER) as i32;
    let black_center = popcount(board.black_pieces & CENTER) as i32;
    let center_bonus = (white_center - black_center) * 20;
    
    // Retorna do ponto de vista das brancas
    if board.to_move == Color::White {
        material + center_bonus
    } else {
        -(material + center_bonus)
    }
}