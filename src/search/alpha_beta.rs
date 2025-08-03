use crate::core::*;
use crate::engine::TranspositionTable;
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
        
        // Ordenação básica de movimentos
        let mut ordered_moves = moves;
        self.order_moves_basic(board, &mut ordered_moves);
        
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
        self.order_moves_basic(board, &mut ordered_moves);
        
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
            return (self.quiescence_search(board, alpha, beta, 6), Vec::new());
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
        
        // Ordena movimentos (inclui TT move se disponível)
        let mut ordered_moves = moves;
        self.order_moves_with_tt(board, &mut ordered_moves, shared_tt);
        
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
                    break;  // Alpha-beta cutoff
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
    
    
    /// Ordenação com TT move prioritário + MVV-LVA + heurísticas
    fn order_moves_with_tt(&self, board: &Board, moves: &mut Vec<Move>, shared_tt: &Arc<Mutex<TranspositionTable>>) {
        // Busca TT move para priorizar
        let tt_move = if let Ok(mut tt_guard) = shared_tt.try_lock() {
            tt_guard.get_best_move(board.zobrist_hash)
        } else {
            None
        };
        
        moves.sort_by_key(|&mv| {
            let mut score = 0;
            
            // 0. TT MOVE - máxima prioridade
            if let Some(tt_mv) = tt_move {
                if mv.from == tt_mv.from && mv.to == tt_mv.to && mv.promotion == tt_mv.promotion {
                    score += 100000;  // Prioridade máxima
                }
            }
            
            // 1. MVV-LVA COMPLETO para capturas
            if self.is_capture(board, mv) {
                let victim_value = self.get_piece_value(board, mv.to);
                let attacker_value = self.get_piece_value(board, mv.from);
                
                // MVV-LVA: Most Valuable Victim - Least Valuable Attacker
                score += 10000 + (victim_value * 10) - attacker_value;
                
                // Bônus para capturas que ganham material
                if victim_value >= attacker_value {
                    score += 1000;
                }
            }
            
            // 2. Promoções (especialmente para dama)
            if let Some(promotion) = mv.promotion {
                score += match promotion {
                    PieceKind::Queen => 9000,
                    PieceKind::Rook => 5000,
                    PieceKind::Bishop => 3300,
                    PieceKind::Knight => 3200,
                    _ => 1000,
                };
            }
            
            // 3. Xeques (podem forçar respostas)
            if self.gives_check_fast(board, mv) {
                score += 200;
            }
            
            // 4. Controle do centro expandido
            score += self.center_control_score(mv);
            
            // 5. Desenvolvimento de peças
            if self.is_development_move(board, mv) {
                score += 100;
            }
            
            // 6. Roque (segurança do rei)
            if mv.is_castling {
                score += 150;
            }
            
            -score // Ordem decrescente
        });
    }
    
    /// Ordenação avançada MVV-LVA + heurísticas (sem TT)
    fn order_moves_basic(&self, board: &Board, moves: &mut Vec<Move>) {
        moves.sort_by_key(|&mv| {
            let mut score = 0;
            
            // 1. MVV-LVA COMPLETO para capturas
            if self.is_capture(board, mv) {
                let victim_value = self.get_piece_value(board, mv.to);
                let attacker_value = self.get_piece_value(board, mv.from);
                
                // MVV-LVA: Most Valuable Victim - Least Valuable Attacker
                score += 10000 + (victim_value * 10) - attacker_value;
                
                // Bônus para capturas que ganham material
                if victim_value >= attacker_value {
                    score += 1000;
                }
            }
            
            // 2. Promoções (especialmente para dama)
            if let Some(promotion) = mv.promotion {
                score += match promotion {
                    PieceKind::Queen => 9000,
                    PieceKind::Rook => 5000,
                    PieceKind::Bishop => 3300,
                    PieceKind::Knight => 3200,
                    _ => 1000,
                };
            }
            
            // 3. Xeques (podem forçar respostas)
            if self.gives_check_fast(board, mv) {
                score += 200;
            }
            
            // 4. Controle do centro expandido
            score += self.center_control_score(mv);
            
            // 5. Desenvolvimento de peças
            if self.is_development_move(board, mv) {
                score += 100;
            }
            
            // 6. Roque (segurança do rei)
            if mv.is_castling {
                score += 150;
            }
            
            -score // Ordem decrescente
        });
    }
    
    /// Busca Quiescente - explora capturas para evitar horizon effect
    fn quiescence_search(&self, board: &Board, mut alpha: i32, beta: i32, qs_depth: u8) -> i32 {
        self.nodes_searched.fetch_add(1, Ordering::Relaxed);
        
        if self.should_stop() || qs_depth == 0 {
            return evaluate_position(board);
        }
        
        // Stand-pat: avaliação estática
        let stand_pat = evaluate_position(board);
        
        // Beta cutoff
        if stand_pat >= beta {
            return beta;
        }
        
        // Melhora alpha
        if stand_pat > alpha {
            alpha = stand_pat;
        }
        
        // Delta pruning: se mesmo capturando a dama não melhora significativamente
        if stand_pat + 1000 < alpha {
            return alpha; // Posição muito ruim
        }
        
        // Gera apenas capturas e promoções
        let tactical_moves = self.generate_tactical_moves(board);
        
        if tactical_moves.is_empty() {
            return stand_pat;
        }
        
        let mut best_score = stand_pat;
        
        for mv in tactical_moves {
            // SEE (Static Exchange Evaluation) básico
            if !self.is_good_capture(board, mv) {
                continue;
            }
            
            let mut board_clone = *board;
            let undo_info = board_clone.make_move_with_undo(mv);
            let previous_to_move = !board_clone.to_move;
            
            if !board_clone.is_king_in_check(previous_to_move) {
                let score = -self.quiescence_search(&board_clone, -beta, -alpha, qs_depth - 1);
                
                board_clone.unmake_move(mv, undo_info);
                
                if score > best_score {
                    best_score = score;
                }
                
                if score > alpha {
                    alpha = score;
                }
                
                // Beta cutoff
                if score >= beta {
                    return beta;
                }
            } else {
                board_clone.unmake_move(mv, undo_info);
            }
        }
        
        best_score
    }
    
    /// Gera apenas movimentos táticos (capturas, promoções)
    fn generate_tactical_moves(&self, board: &Board) -> Vec<Move> {
        let all_moves = board.generate_legal_moves();
        let mut tactical_moves = Vec::new();
        
        for mv in all_moves {
            // Capturas
            if self.is_capture(board, mv) {
                tactical_moves.push(mv);
            }
            // Promoções
            else if mv.promotion.is_some() {
                tactical_moves.push(mv);
            }
        }
        
        // Ordena capturas por MVV-LVA
        tactical_moves.sort_by_key(|&mv| {
            if self.is_capture(board, mv) {
                let victim_value = self.get_piece_value(board, mv.to);
                let attacker_value = self.get_piece_value(board, mv.from);
                -(victim_value * 10 - attacker_value)
            } else {
                0
            }
        });
        
        tactical_moves
    }
    
    /// Verifica se captura é boa (SEE simplificado)
    fn is_good_capture(&self, board: &Board, mv: Move) -> bool {
        if !self.is_capture(board, mv) {
            return mv.promotion.is_some(); // Promoções sempre boas
        }
        
        let victim_value = self.get_piece_value(board, mv.to);
        let attacker_value = self.get_piece_value(board, mv.from);
        
        // Se vítima vale mais ou igual, provavelmente boa
        if victim_value >= attacker_value {
            return true;
        }
        
        // Se diferença muito grande, provavelmente ruim
        if victim_value + 200 < attacker_value {
            return false;
        }
        
        true // Casos duvidosos, deixa quiescence decidir
    }
    
    /// Verifica se dá xeque (versão rápida)
    fn gives_check_fast(&self, board: &Board, mv: Move) -> bool {
        // Implementação simplificada - poderia ser muito otimizada
        let mut test_board = *board;
        if test_board.make_move(mv) {
            test_board.is_king_in_check(!board.to_move)
        } else {
            false
        }
    }
    
    /// Score de controle do centro expandido
    fn center_control_score(&self, mv: Move) -> i32 {
        match mv.to {
            27 | 28 | 35 | 36 => 50, // e4, d4, e5, d5 - centro puro
            18 | 19 | 20 | 21 | 26 | 29 | 34 | 37 | 42 | 43 | 44 | 45 => 25, // Centro expandido
            _ => 0
        }
    }
    
    /// Verifica se é movimento de desenvolvimento
    fn is_development_move(&self, board: &Board, mv: Move) -> bool {
        let from_bb = 1u64 << mv.from;
        
        // Cavalos e bispos saindo da fileira inicial
        if (board.knights & from_bb) != 0 || (board.bishops & from_bb) != 0 {
            let initial_rank = if board.to_move == Color::White { 
                mv.from < 16 
            } else { 
                mv.from > 47 
            };
            return initial_rank;
        }
        
        false
    }
    
    fn is_capture(&self, board: &Board, mv: Move) -> bool {
        let to_bb = 1u64 << mv.to;
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };
        (enemy_pieces & to_bb) != 0 || mv.is_en_passant
    }
    
    fn get_piece_value(&self, board: &Board, square: u8) -> i32 {
        let square_bb = 1u64 << square;
        if (board.pawns & square_bb) != 0 { 100 }
        else if (board.knights & square_bb) != 0 { 320 }
        else if (board.bishops & square_bb) != 0 { 330 }
        else if (board.rooks & square_bb) != 0 { 500 }
        else if (board.queens & square_bb) != 0 { 900 }
        else { 0 }
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
        
        // PV line
        let pv = if result.pv_line.is_empty() {
            String::new()
        } else {
            let pv_moves: Vec<String> = result.pv_line.iter()
                .map(|mv| self.format_uci_move(*mv))
                .collect();
            format!(" pv {}", pv_moves.join(" "))
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

/// Avaliação simples de posição
fn evaluate_position(board: &Board) -> i32 {
    use crate::utils::*;
    
    // Material
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
    
    let material = (white_pawns - black_pawns) * 100 +
                  (white_knights - black_knights) * 320 +
                  (white_bishops - black_bishops) * 330 +
                  (white_rooks - black_rooks) * 500 +
                  (white_queens - black_queens) * 900;
    
    // Controle do centro
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