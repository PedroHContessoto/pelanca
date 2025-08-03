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
    
    // Usa order_moves do módulo dedicado para consistência
    order_moves(board, &mut tactical_moves, Some(tt));
    
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



/// Verifica se movimento dá xeque usando make/unmake do board
fn gives_check_fast(board: &Board, mv: Move) -> bool {
    let mut test_board = *board;
    let undo_info = test_board.make_move_with_undo(mv);
    let previous_to_move = !test_board.to_move;
    
    let gives_check = !test_board.is_king_in_check(previous_to_move) && 
                      test_board.is_king_in_check(test_board.to_move);
    
    test_board.unmake_move(mv, undo_info);
    gives_check
}

/// Obtém valor da peça em uma casa usando método do board
fn get_piece_value(board: &Board, square: u8) -> i32 {
    if let Some(piece) = board.get_piece_at(square) {
        if piece.kind == PieceKind::King {
            10000 // Valor especial para quiescence
        } else {
            piece.kind.value()
        }
    } else {
        0
    }
}