use crate::core::*;
use crate::engine::TranspositionTable;
use crate::search::move_ordering::order_moves;
use crate::search::see::SEE;
use std::sync::atomic::{AtomicU64, Ordering};

/// Busca Quiescente - explora capturas além da profundidade limite
pub fn quiescence_search(
    board: &Board,
    mut alpha: i32,
    beta: i32,
    qs_depth: u8,
    nodes_searched: &AtomicU64,
    tt: &mut TranspositionTable,
    evaluate_fn: fn(&Board) -> i32
) -> i32 {
    nodes_searched.fetch_add(1, Ordering::Relaxed);
    
    // Limite de profundidade quiescente
    if qs_depth == 0 {
        return evaluate_fn(board);
    }
    
    // Stand-pat: avaliação estática
    let stand_pat = evaluate_fn(board);
    
    // Beta cutoff
    if stand_pat >= beta {
        return beta;
    }
    
    // Melhora alpha
    if stand_pat > alpha {
        alpha = stand_pat;
    }
    
    // Delta pruning: se mesmo capturando a dama não melhora alpha significativamente
    const QUEEN_VALUE: i32 = 900;
    if stand_pat + QUEEN_VALUE + 200 < alpha {
        return alpha; // Posição muito ruim, nem capturas ajudam
    }
    
    // Gera apenas movimentos táticos (capturas, promoções, xeques em xeque)
    let mut tactical_moves = generate_tactical_moves(board);
    
    if tactical_moves.is_empty() {
        return stand_pat;
    }
    
    // Ordena capturas por MVV-LVA
    order_tactical_moves(board, &mut tactical_moves);
    
    let mut best_score = stand_pat;
    
    for mv in tactical_moves {
        // SEE (Static Exchange Evaluation) pruning - usa módulo dedicado
        if SEE::quick_capture_eval(board, mv) < -50 {
            continue; // Pula capturas claramente ruins
        }
        
        let mut board_clone = *board;
        let undo_info = board_clone.make_move_with_undo(mv);
        let previous_to_move = !board_clone.to_move;
        
        if !board_clone.is_king_in_check(previous_to_move) {
            let score = -quiescence_search(
                &board_clone, 
                -beta, 
                -alpha, 
                qs_depth - 1, 
                nodes_searched,
                tt,
                evaluate_fn
            );
            
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

/// Gera apenas movimentos táticos para quiescence
fn generate_tactical_moves(board: &Board) -> Vec<Move> {
    let all_moves = board.generate_legal_moves();
    let mut tactical_moves = Vec::new();
    
    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };
    
    for mv in all_moves {
        let to_bb = 1u64 << mv.to;
        
        // Capturas
        if (enemy_pieces & to_bb) != 0 {
            tactical_moves.push(mv);
            continue;
        }
        
        // En passant
        if mv.is_en_passant {
            tactical_moves.push(mv);
            continue;
        }
        
        // Promoções
        if mv.promotion.is_some() {
            tactical_moves.push(mv);
            continue;
        }
        
        // Xeques apenas se o rei inimigo já estiver em xeque (evitar horizon effect)
        if board.is_king_in_check(board.to_move) && gives_check_fast(board, mv) {
            tactical_moves.push(mv);
        }
    }
    
    tactical_moves
}

/// Ordena movimentos táticos por MVV-LVA
fn order_tactical_moves(board: &Board, moves: &mut Vec<Move>) {
    moves.sort_by_key(|&mv| {
        let mut score = 0;
        
        // MVV-LVA para capturas
        let to_bb = 1u64 << mv.to;
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };
        
        if (enemy_pieces & to_bb) != 0 || mv.is_en_passant {
            let victim_value = get_piece_value(board, mv.to);
            let attacker_value = get_piece_value(board, mv.from);
            score += 1000 + victim_value - attacker_value / 10;
        }
        
        // Promoções
        if let Some(promotion) = mv.promotion {
            score += match promotion {
                PieceKind::Queen => 900,
                PieceKind::Rook => 500,
                PieceKind::Bishop => 330,
                PieceKind::Knight => 320,
                _ => 100,
            };
        }
        
        -score // Ordem decrescente
    });
}

/// Verifica se captura vale a pena (SEE simplificado)
fn is_capture_worthwhile(board: &Board, mv: Move) -> bool {
    let to_bb = 1u64 << mv.to;
    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };
    
    // Se não é captura, aceita (promoção, xeque)
    if (enemy_pieces & to_bb) == 0 && !mv.is_en_passant {
        return true;
    }
    
    let victim_value = get_piece_value(board, mv.to);
    let attacker_value = get_piece_value(board, mv.from);
    
    // SEE simplificado: se vítima vale mais que atacante, provavelmente vale a pena
    if victim_value >= attacker_value {
        return true;
    }
    
    // Para capturas ruins (peça valiosa captura peça barata), faz verificação mais detalhada
    if victim_value + 200 < attacker_value {
        return false; // Claramente ruim
    }
    
    true // Casos duvidosos, deixa a busca decidir
}

/// Verifica se movimento dá xeque (versão rápida)
fn gives_check_fast(board: &Board, mv: Move) -> bool {
    // Implementação simplificada - em produção seria otimizada
    let mut test_board = *board;
    if test_board.make_move(mv) {
        let enemy_color = !board.to_move;
        test_board.is_king_in_check(enemy_color)
    } else {
        false
    }
}

/// Obtém valor da peça em uma casa
fn get_piece_value(board: &Board, square: u8) -> i32 {
    let square_bb = 1u64 << square;
    
    if (board.pawns & square_bb) != 0 { 100 }
    else if (board.knights & square_bb) != 0 { 320 }
    else if (board.bishops & square_bb) != 0 { 330 }
    else if (board.rooks & square_bb) != 0 { 500 }
    else if (board.queens & square_bb) != 0 { 900 }
    else if (board.kings & square_bb) != 0 { 10000 }
    else { 0 }
}