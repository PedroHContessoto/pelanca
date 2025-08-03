// Busca de quiescência para evitar efeito horizonte

use super::*;
use crate::core::*;
use std::sync::Arc;

/// Busca de quiescência - avalia apenas capturas para estabilizar a avaliação
pub fn quiescence_search(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    ply: i32,
    controller: &Arc<SearchController>
) -> i32 {
    // Verifica parada
    if controller.should_stop() {
        return 0;
    }

    controller.increment_nodes(1);

    // Avaliação estática como baseline
    let stand_pat = evaluate(board);

    // Beta cutoff
    if stand_pat >= beta {
        return beta;
    }

    // Delta pruning - se estamos muito abaixo de alpha, apenas capturas muito boas ajudariam
    const DELTA_MARGIN: i32 = 900; // Valor de uma rainha
    if stand_pat < alpha - DELTA_MARGIN {
        return alpha;
    }

    // Atualiza alpha
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Profundidade máxima da quiescência
    if ply > 32 {
        return stand_pat;
    }

    // Gera apenas capturas e promoções
    let moves = generate_captures_and_promotions(board);

    // Se não há capturas, retorna avaliação estática
    if moves.is_empty() {
        return stand_pat;
    }

    // Ordena capturas por MVV-LVA
    let mut ordered_moves = moves;
    order_captures(board, &mut ordered_moves);

    // Busca apenas capturas promissoras
    for &mv in &ordered_moves {
        // SEE pruning - pula capturas ruins
        if !is_promotion(mv) && see_capture(board, mv) < 0 {
            continue;
        }

        let undo_info = board.make_move_with_undo(mv);

        if !board.is_king_in_check(!board.to_move) {
            let score = -quiescence_search(board, -beta, -alpha, ply + 1, controller);

            board.unmake_move(mv, undo_info);

            if score >= beta {
                return beta; // Beta cutoff
            }

            if score > alpha {
                alpha = score;
            }
        } else {
            board.unmake_move(mv, undo_info);
        }

        if controller.should_stop() {
            break;
        }
    }

    alpha
}

/// Gera apenas capturas e promoções para busca de quiescência
fn generate_captures_and_promotions(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(32);

    // Gera capturas de peões (incluindo en passant)
    moves.extend(crate::moves::pawn::generate_pawn_captures(board));

    // Gera capturas de outras peças
    let our_pieces = if board.to_move == Color::White {
        board.white_pieces
    } else {
        board.black_pieces
    };

    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };

    let all_pieces = board.white_pieces | board.black_pieces;

    // Cavalos
    let mut knights = board.knights & our_pieces;
    while knights != 0 {
        let from = knights.trailing_zeros() as u8;
        let attacks = crate::moves::knight::get_knight_attacks(from) & enemy_pieces;
        add_moves_from_bitboard(&mut moves, from, attacks);
        knights &= knights - 1;
    }

    // Bispos
    let mut bishops = board.bishops & our_pieces;
    while bishops != 0 {
        let from = bishops.trailing_zeros() as u8;
        let attacks = crate::moves::sliding::get_bishop_attacks(from, all_pieces) & enemy_pieces;
        add_moves_from_bitboard(&mut moves, from, attacks);
        bishops &= bishops - 1;
    }

    // Torres
    let mut rooks = board.rooks & our_pieces;
    while rooks != 0 {
        let from = rooks.trailing_zeros() as u8;
        let attacks = crate::moves::sliding::get_rook_attacks(from, all_pieces) & enemy_pieces;
        add_moves_from_bitboard(&mut moves, from, attacks);
        rooks &= rooks - 1;
    }

    // Rainhas
    let mut queens = board.queens & our_pieces;
    while queens != 0 {
        let from = queens.trailing_zeros() as u8;
        let attacks = crate::moves::queen::get_queen_attacks(from, all_pieces) & enemy_pieces;
        add_moves_from_bitboard(&mut moves, from, attacks);
        queens &= queens - 1;
    }

    // Rei (apenas capturas)
    let mut kings = board.kings & our_pieces;
    if kings != 0 {
        let from = kings.trailing_zeros() as u8;
        let attacks = crate::moves::king::get_king_attacks(from) & enemy_pieces;
        add_moves_from_bitboard(&mut moves, from, attacks);
    }

    moves
}

/// Adiciona movimentos de um bitboard de destinos
fn add_moves_from_bitboard(moves: &mut Vec<Move>, from: u8, mut targets: Bitboard) {
    while targets != 0 {
        let to = targets.trailing_zeros() as u8;
        moves.push(Move {
            from,
            to,
            promotion: None,
            is_castling: false,
            is_en_passant: false,
        });
        targets &= targets - 1;
    }
}

/// Verifica se é uma promoção
fn is_promotion(mv: Move) -> bool {
    mv.promotion.is_some()
}

/// Busca de quiescência específica para finais de jogo
pub fn endgame_quiescence(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    ply: i32,
    controller: &Arc<SearchController>
) -> i32 {
    // Em finais, também considera movimentos que dão xeque
    if controller.should_stop() {
        return 0;
    }

    controller.increment_nodes(1);

    // Verifica se é posição terminal
    if board.is_checkmate() {
        return -MATE_SCORE + ply;
    }

    if board.is_stalemate() || board.is_draw_by_insufficient_material() || board.is_draw_by_50_moves() {
        return DRAW_SCORE;
    }

    let stand_pat = evaluate(board);

    if stand_pat >= beta {
        return beta;
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    if ply > 64 {
        return stand_pat;
    }

    // Em finais, também gera movimentos que dão xeque
    let mut moves = generate_captures_and_promotions(board);

    // Adiciona movimentos de xeque (simplificado)
    if board.is_king_in_check(board.to_move) {
        // Se em xeque, precisa gerar todos os movimentos
        moves = board.generate_all_moves();
    }

    if moves.is_empty() {
        return stand_pat;
    }

    // Ordena movimentos
    order_moves(board, &mut moves, None, ply, controller);

    let mut legal_moves = 0;

    for &mv in &moves {
        let undo_info = board.make_move_with_undo(mv);

        if !board.is_king_in_check(!board.to_move) {
            legal_moves += 1;

            let score = -endgame_quiescence(board, -beta, -alpha, ply + 1, controller);

            board.unmake_move(mv, undo_info);

            if score >= beta {
                return beta;
            }

            if score > alpha {
                alpha = score;
            }

            // Em finais, limita número de movimentos explorados
            if legal_moves >= 10 && !board.is_king_in_check(board.to_move) {
                break;
            }
        } else {
            board.unmake_move(mv, undo_info);
        }

        if controller.should_stop() {
            break;
        }
    }

    // Se não há movimentos legais e estamos em xeque, é mate
    if legal_moves == 0 && board.is_king_in_check(board.to_move) {
        return -MATE_SCORE + ply;
    }

    alpha
}