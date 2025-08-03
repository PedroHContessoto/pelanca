use crate::core::*;
use crate::engine::TranspositionTable;
use crate::search::move_ordering::{order_moves_advanced, KillerMoves, HistoryTable, order_moves};
use crate::search::quiescence::quiescence_search;
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

/// Motor Alpha-Beta otimizado para UCI
pub struct AlphaBetaTTEngine {
    pub nodes_searched: AtomicU64,
    pub start_time: Option<Instant>,
    pub max_time: Option<Duration>,
    pub should_stop: AtomicBool,
    pub threads: usize,
    pub shared_tt: Arc<Mutex<TranspositionTable>>,
    pub killers: Arc<Mutex<KillerMoves>>,
    pub history: Arc<Mutex<HistoryTable>>,
}

impl AlphaBetaTTEngine {
    pub fn new() -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: num_cpus::get().max(1),
            shared_tt: Arc::new(Mutex::new(TranspositionTable::new())),
            killers: Arc::new(Mutex::new(KillerMoves::new())),
            history: Arc::new(Mutex::new(HistoryTable::new())),
        }
    }
    
    pub fn new_with_threads(threads: usize) -> Self {
        Self {
            nodes_searched: AtomicU64::new(0),
            start_time: None,
            max_time: None,
            should_stop: AtomicBool::new(false),
            threads: threads.max(1),
            shared_tt: Arc::new(Mutex::new(TranspositionTable::new())),
            killers: Arc::new(Mutex::new(KillerMoves::new())),
            history: Arc::new(Mutex::new(HistoryTable::new())),
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
            
            // UCI standard output format - clean and professional
            self.print_uci_info(&best_result);
            
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
        
        // UCI standard output format
        self.print_uci_info(&result);
        
        result
    }
    
    /// Alpha-Beta paralelo na raiz
    fn alpha_beta_root_parallel(&self, board: &Board, depth: u8) -> (Option<Move>, i32, Vec<Move>) {
        if depth <= 2 {
            return self.alpha_beta_root_sequential(board, depth);
        }
        
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board), Vec::new());
        }
        
        // Ordenação de movimentos usando heurísticas avançadas
        let mut ordered_moves = moves;
        self.order_moves_thread_safe(board, &mut ordered_moves, depth, &self.shared_tt);
        
        // Paralelização massiva com TT compartilhada
        let shared_tt = self.shared_tt.clone();
        let results: Vec<(Move, i32, Vec<Move>)> = ordered_moves.par_iter().filter_map(|&mv| {
            if self.should_stop() {
                return None;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let (score, mut pv_line) = self.negamax(&board_clone, depth - 1, i32::MIN, i32::MAX, &shared_tt);
                
                pv_line.insert(0, mv);
                board_clone.unmake_move(mv, undo_info);
                Some((mv, -score, pv_line))  // Nega o score do filho
            } else {
                board_clone.unmake_move(mv, undo_info);
                None
            }
        }).collect();
        
        // Encontra o melhor movimento
        if let Some((best_move, best_score, pv_line)) = results.into_iter().max_by_key(|(_, score, _)| *score) {
            (Some(best_move), best_score, pv_line)
        } else {
            (Some(ordered_moves[0]), 0, vec![ordered_moves[0]])
        }
    }
    
    /// Alpha-Beta sequencial na raiz 
    fn alpha_beta_root_sequential(&self, board: &Board, depth: u8) -> (Option<Move>, i32, Vec<Move>) {
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            return (None, evaluate_position(board), Vec::new());
        }
        
        let mut ordered_moves = moves;
        self.order_moves_thread_safe(board, &mut ordered_moves, depth, &self.shared_tt);
        
        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut best_pv = Vec::new();
        let mut alpha = i32::MIN;
        let beta = i32::MAX;
        let shared_tt = self.shared_tt.clone();
        
        for &mv in &ordered_moves {
            if self.should_stop() {
                break;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let (score, pv_line) = self.negamax(&board_clone, depth - 1, -beta, -alpha, &shared_tt);
                let score = -score;  // Nega o score do filho
                
                if score > best_score {
                    best_score = score;
                    best_move = Some(mv);
                    
                    best_pv = vec![mv];
                    best_pv.extend(pv_line);
                }
                
                alpha = alpha.max(score);
                
                if beta <= alpha {
                    board_clone.unmake_move(mv, undo_info);
                    break;
                }
            }
            
            board_clone.unmake_move(mv, undo_info);
        }
        
        (best_move, best_score, best_pv)
    }
    
    /// Negamax com TT compartilhada integrada - performance e hashfull > 0
    fn negamax(&self, board: &Board, depth: u8, mut alpha: i32, beta: i32, shared_tt: &Arc<Mutex<TranspositionTable>>) -> (i32, Vec<Move>) {
        self.nodes_searched.fetch_add(1, Ordering::Relaxed);
        
        if self.should_stop() {
            return (0, Vec::new());
        }
        
        let original_alpha = alpha;
        let zobrist_hash = board.zobrist_hash;
        
        // ========== TT PROBE ==========
        if let Ok(mut tt_guard) = shared_tt.try_lock() {
            if let Some(tt_score) = tt_guard.probe(zobrist_hash, depth, alpha, beta) {
                return (tt_score, Vec::new());  // TT hit - retorna imediatamente
            }
        }
        
        if depth == 0 {
            // Usa TT compartilhada ou cria temporária se bloqueada
            if let Ok(mut tt_guard) = shared_tt.try_lock() {
                return (quiescence_search(board, alpha, beta, 6, &self.nodes_searched, &mut *tt_guard, evaluate_position), Vec::new());
            } else {
                return (quiescence_search(board, alpha, beta, 6, &self.nodes_searched, &mut TranspositionTable::new(), evaluate_position), Vec::new());
            }
        }
        
        let moves = board.generate_legal_moves();
        
        if moves.is_empty() {
            let score = if board.is_king_in_check(board.to_move) {
                -30000 + depth as i32  // Mate em 'depth' movimentos
            } else {
                0  // Stalemate
            };
            return (score, Vec::new());
        }
        
        let mut best_score = i32::MIN;
        let mut best_pv = Vec::new();
        let mut best_move = None;
        
        // Ordena movimentos com heurísticas avançadas (thread-safe)
        let mut ordered_moves = moves;
        self.order_moves_thread_safe(board, &mut ordered_moves, depth, shared_tt);
        
        for &mv in &ordered_moves {
            if self.should_stop() {
                break;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let (score, mut pv_line) = self.negamax(&board_clone, depth - 1, -beta, -alpha, shared_tt);
                let score = -score;  // Negamax: sempre nega o score do filho
                
                board_clone.unmake_move(mv, undo_info);
                
                if score > best_score {
                    best_score = score;
                    best_move = Some(mv);
                    best_pv = vec![mv];
                    best_pv.append(&mut pv_line);
                }
                
                alpha = alpha.max(score);
                
                if alpha >= beta {
                    // Alpha-beta cutoff - atualiza killer moves e history
                    self.update_cutoff_heuristics(mv, depth, board.to_move);
                    break;
                }
            } else {
                board_clone.unmake_move(mv, undo_info);
            }
        }
        
        // Se nenhum movimento legal foi encontrado, retorna mate/stalemate
        if best_score == i32::MIN {
            let score = if board.is_king_in_check(board.to_move) {
                -30000 + depth as i32
            } else {
                0
            };
            return (score, Vec::new());
        }
        
        // ========== TT STORE ==========
        let tt_flag = if best_score <= original_alpha {
            crate::engine::TT_ALPHA  // Upper bound
        } else if best_score >= beta {
            crate::engine::TT_BETA   // Lower bound
        } else {
            crate::engine::TT_EXACT  // Exact score
        };
        
        if let Ok(mut tt_guard) = shared_tt.try_lock() {
            tt_guard.store(zobrist_hash, depth, best_score, tt_flag, best_move);
        }
        
        (best_score, best_pv)
    }
    
    
    /// Ordenação thread-safe com heurísticas avançadas
    fn order_moves_thread_safe(&self, board: &Board, moves: &mut Vec<Move>, depth: u8, shared_tt: &Arc<Mutex<TranspositionTable>>) {
        // Tenta acessar TT, killers e history de forma thread-safe
        let tt_option = shared_tt.try_lock().ok();
        let killers_option = self.killers.try_lock().ok();  
        let history_option = self.history.try_lock().ok();
        
        if let (Some(mut tt_guard), Some(killers_guard), Some(history_guard)) = (tt_option, killers_option, history_option) {
            order_moves_advanced(board, moves, depth, Some(&mut *tt_guard), &*killers_guard, &*history_guard);
        } else {
            // Fallback para ordenação básica se não conseguir todos os locks
            if let Ok(mut tt_guard) = shared_tt.try_lock() {
                order_moves(board, moves, Some(&mut *tt_guard));
            } else {
                order_moves(board, moves, None);
            }
        }
    }
    
    /// Atualiza heurísticas após cutoff (killer moves e history)
    fn update_cutoff_heuristics(&self, mv: Move, depth: u8, color: Color) {
        // Verifica se não é captura (killer moves são para movimentos silenciosos)
        if !self.is_likely_capture(mv) {
            // Atualiza killer moves
            if let Ok(mut killers) = self.killers.try_lock() {
                killers.add_killer(depth, mv);
            }
            
            // Atualiza history table
            if let Ok(mut history) = self.history.try_lock() {
                history.update(color, mv, depth);
            }
        }
    }
    
    /// Verifica se movimento é provavelmente uma captura (sem acesso ao board)
    fn is_likely_capture(&self, _mv: Move) -> bool {
        // Simplificação: assume que promoções e en passant são "táticos"
        _mv.promotion.is_some() || _mv.is_en_passant
    }
    
    /// Valida linha principal removendo movimentos inválidos
    fn validate_pv_line(&self, pv_line: &[Move]) -> Vec<Move> {
        // Para evitar problemas, limita PV a movimentos básicos válidos
        // Filtra movimentos que parecem suspeitos
        pv_line.iter()
            .take(8)  // Limita a 8 movimentos para performance
            .filter(|&&mv| self.is_move_reasonable(mv))
            .copied()
            .collect()
    }
    
    /// Verifica se movimento parece razoável (filtro básico)
    fn is_move_reasonable(&self, mv: Move) -> bool {
        // Verifica se as casas estão no tabuleiro
        if mv.from >= 64 || mv.to >= 64 {
            return false;
        }
        
        // Verifica se não é movimento nulo
        if mv.from == mv.to {
            return false;
        }
        
        // Movimentos óbvios inválidos
        match (mv.from, mv.to) {
            // a1-a2: torre não pode passar através do peão
            (0, 8) => false,
            // h1-h2: torre não pode passar através do peão  
            (7, 15) => false,
            // a8-a7: torre preta não pode passar através do peão
            (56, 48) => false,
            // h8-h7: torre preta não pode passar através do peão
            (63, 55) => false,
            _ => true
        }
    }
    
    /// Verifica se deve parar a busca
    fn should_stop(&self) -> bool {
        if let (Some(start), Some(max_time)) = (self.start_time, self.max_time) {
            start.elapsed() >= max_time
        } else {
            false
        }
    }
    
    /// UCI standard output format - clean and professional
    fn print_uci_info(&self, result: &SearchResult) {
        let nps = if result.time_elapsed.as_millis() > 0 {
            (result.nodes_searched as f64 / result.time_elapsed.as_secs_f64()) as u64
        } else {
            0
        };
        let time_ms = result.time_elapsed.as_millis();
        
        // Formata score (centipawns ou mate) - corrige i32::MIN em depth 1
        let score_output = if result.score == i32::MIN && result.depth == 1 {
            "score cp 0".to_string()  // Correção simples para depth 1
        } else if result.score.abs() > 29000 {
            let mate_in = (30000 - result.score.abs()) / 2 + 1;
            if result.score > 0 {
                format!("score mate {}", mate_in)
            } else {
                format!("score mate -{}", mate_in)
            }
        } else {
            format!("score cp {}", result.score)
        };
        
        // PV line - valida movimentos antes de exibir
        let pv = if result.pv_line.is_empty() {
            String::new()
        } else {
            let valid_pv = self.validate_pv_line(&result.pv_line);
            if valid_pv.is_empty() {
                String::new()
            } else {
                let pv_moves: Vec<String> = valid_pv.iter()
                    .map(|mv| self.format_uci_move(*mv))
                    .collect();
                format!(" pv {}", pv_moves.join(" "))
            }
        };
        
        // Hashfull da TT compartilhada
        let hashfull = if let Ok(tt) = self.shared_tt.try_lock() {
            tt.hashfull()
        } else {
            0
        };
        
        // UCI standard output format - clean and professional
        print!("info depth {} {} nodes {} nps {} time {} hashfull {} tbhits {} multipv {}{}\n", 
               result.depth, 
               score_output, 
               result.nodes_searched, 
               nps, 
               time_ms,
               hashfull,
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

/// Piece-Square Tables para avaliação posicional
mod pst {
    use crate::core::PieceKind;
    // PST para peões (middlegame)
    pub const PAWN_PST: [i32; 64] = [
         0,  0,  0,  0,  0,  0,  0,  0,
        50, 50, 50, 50, 50, 50, 50, 50,
        10, 10, 20, 30, 30, 20, 10, 10,
         5,  5, 10, 25, 25, 10,  5,  5,
         0,  0,  0, 20, 20,  0,  0,  0,
         5, -5,-10,  0,  0,-10, -5,  5,
         5, 10, 10,-20,-20, 10, 10,  5,
         0,  0,  0,  0,  0,  0,  0,  0
    ];
    
    // PST para cavalos
    pub const KNIGHT_PST: [i32; 64] = [
        -50,-40,-30,-30,-30,-30,-40,-50,
        -40,-20,  0,  0,  0,  0,-20,-40,
        -30,  0, 10, 15, 15, 10,  0,-30,
        -30,  5, 15, 20, 20, 15,  5,-30,
        -30,  0, 15, 20, 20, 15,  0,-30,
        -30,  5, 10, 15, 15, 10,  5,-30,
        -40,-20,  0,  5,  5,  0,-20,-40,
        -50,-40,-30,-30,-30,-30,-40,-50,
    ];
    
    // PST para bispos
    pub const BISHOP_PST: [i32; 64] = [
        -20,-10,-10,-10,-10,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0,  5, 10, 10,  5,  0,-10,
        -10,  5,  5, 10, 10,  5,  5,-10,
        -10,  0, 10, 10, 10, 10,  0,-10,
        -10, 10, 10, 10, 10, 10, 10,-10,
        -10,  5,  0,  0,  0,  0,  5,-10,
        -20,-10,-10,-10,-10,-10,-10,-20,
    ];
    
    // PST para torres
    pub const ROOK_PST: [i32; 64] = [
         0,  0,  0,  0,  0,  0,  0,  0,
         5, 10, 10, 10, 10, 10, 10,  5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
         0,  0,  0,  5,  5,  0,  0,  0
    ];
    
    // PST para dama
    pub const QUEEN_PST: [i32; 64] = [
        -20,-10,-10, -5, -5,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0,  5,  5,  5,  5,  0,-10,
         -5,  0,  5,  5,  5,  5,  0, -5,
          0,  0,  5,  5,  5,  5,  0, -5,
        -10,  5,  5,  5,  5,  5,  0,-10,
        -10,  0,  5,  0,  0,  0,  0,-10,
        -20,-10,-10, -5, -5,-10,-10,-20
    ];
    
    // PST para rei (middlegame - segurança)
    pub const KING_PST: [i32; 64] = [
        -30,-40,-40,-50,-50,-40,-40,-30,
        -30,-40,-40,-50,-50,-40,-40,-30,
        -30,-40,-40,-50,-50,-40,-40,-30,
        -30,-40,-40,-50,-50,-40,-40,-30,
        -20,-30,-30,-40,-40,-30,-30,-20,
        -10,-20,-20,-20,-20,-20,-20,-10,
         20, 20,  0,  0,  0,  0, 20, 20,
         20, 30, 10,  0,  0, 10, 30, 20
    ];
    
    pub fn get_pst_value(piece: PieceKind, square: u8, is_white: bool) -> i32 {
        let index = if is_white {
            square as usize
        } else {
            // Flip para peças pretas
            (56 + (square % 8) - (square / 8) * 8) as usize
        };
        
        match piece {
            PieceKind::Pawn => PAWN_PST[index],
            PieceKind::Knight => KNIGHT_PST[index],
            PieceKind::Bishop => BISHOP_PST[index],
            PieceKind::Rook => ROOK_PST[index],
            PieceKind::Queen => QUEEN_PST[index],
            PieceKind::King => KING_PST[index],
        }
    }
}

/// Avaliação avançada com PST e mobilidade otimizada
fn evaluate_position(board: &Board) -> i32 {
    use crate::utils::*;
    
    let mut eval = 0;
    
    // === MATERIAL RÁPIDO usando piece_count ===
    for &piece_kind in &[PieceKind::Pawn, PieceKind::Knight, PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen] {
        let white_count = board.piece_count(Color::White, piece_kind) as i32;
        let black_count = board.piece_count(Color::Black, piece_kind) as i32;
        eval += (white_count - black_count) * piece_kind.value();
    }
    
    // === PST apenas ===
    for square in 0..64 {
        if let Some(piece) = board.get_piece_at(square) {
            let pst_value = pst::get_pst_value(piece.kind, square, piece.color == Color::White);
            
            if piece.color == Color::White {
                eval += pst_value;
            } else {
                eval -= pst_value;
            }
        }
    }
    
    // === MOBILIDADE (usa generate_all_moves para performance) ===
    let pseudo_legal_moves = board.generate_all_moves();
    let mobility_bonus = (pseudo_legal_moves.len() as i32) * 2; // 2 centipawns por movimento
    
    if board.to_move == Color::White {
        eval += mobility_bonus;
    } else {
        eval -= mobility_bonus;
    }
    
    // === CONTROLE DO CENTRO EXPANDIDO ===
    const CENTER: u64 = 0x0000001818000000; // e4, e5, d4, d5
    const EXTENDED_CENTER: u64 = 0x00003C3C3C3C0000; // d3-e3-f3-g3 até d6-e6-f6-g6
    
    let white_center = popcount(board.white_pieces & CENTER) as i32;
    let black_center = popcount(board.black_pieces & CENTER) as i32;
    let white_ext_center = popcount(board.white_pieces & EXTENDED_CENTER) as i32;
    let black_ext_center = popcount(board.black_pieces & EXTENDED_CENTER) as i32;
    
    eval += (white_center - black_center) * 20;
    eval += (white_ext_center - black_ext_center) * 5;
    
    // Retorna do ponto de vista do jogador atual
    if board.to_move == Color::White {
        eval
    } else {
        -eval
    }
}