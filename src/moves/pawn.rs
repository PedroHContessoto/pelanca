// Ficheiro: src/moves/pawn.rs
// Descrição: Lógica para gerar os lances dos peões.

use crate::{board::Board, types::{Move, Color, Bitboard, PieceKind}};

// Constantes importadas ou redefinidas para este módulo
const NOT_A_FILE: Bitboard = 0xfefefefefefefefe;
const NOT_H_FILE: Bitboard = 0x7f7f7f7f7f7f7f7f;
const RANK_3: Bitboard = 0x0000000000FF0000;
const RANK_6: Bitboard = 0x0000FF0000000000;

/// Obtém o bitboard de ataques de peão para uma casa específica e cor
#[inline]
pub fn get_pawn_attacks(square: u8, color: Color) -> Bitboard {
    let square_bb = 1u64 << square;
    
    match color {
        Color::White => {
            // Peões brancos atacam diagonalmente para cima
            ((square_bb << 7) & NOT_H_FILE) | ((square_bb << 9) & NOT_A_FILE)
        }
        Color::Black => {
            // Peões pretos atacam diagonalmente para baixo
            ((square_bb >> 7) & NOT_A_FILE) | ((square_bb >> 9) & NOT_H_FILE)
        }
    }
}

/// Gera todos os lances pseudo-legais para os peões do jogador atual.
pub fn generate_pawn_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(16);
    let all_pieces = board.white_pieces | board.black_pieces;

    if board.to_move == Color::White {
        let our_pawns = board.pawns & board.white_pieces;

        // Avanço simples
        let single_push = (our_pawns << 8) & !all_pieces;
        let mut pushes = single_push;
        while pushes != 0 {
            let to_sq = pushes.trailing_zeros() as u8;
            let from_sq = to_sq - 8;
            if to_sq >= 56 { // Promoção
                for piece in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                    moves.push(Move { from: from_sq, to: to_sq, promotion: Some(piece), is_castling: false, is_en_passant: false });
                }
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
            pushes &= pushes - 1;
        }

        // Avanço duplo
        let double_push = ((single_push & RANK_3) << 8) & !all_pieces;
        let mut double_pushes = double_push;
        while double_pushes != 0 {
            let to_sq = double_pushes.trailing_zeros() as u8;
            moves.push(Move { from: to_sq - 16, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            double_pushes &= double_pushes - 1;
        }

        // Adiciona as capturas
        moves.extend(generate_pawn_captures(board));

    } else { // Lances das Pretas
        let our_pawns = board.pawns & board.black_pieces;

        // Avanço simples
        let single_push = (our_pawns >> 8) & !all_pieces;
        let mut pushes = single_push;
        while pushes != 0 {
            let to_sq = pushes.trailing_zeros() as u8;
            let from_sq = to_sq + 8;
            if to_sq <= 7 { // Promoção
                for piece in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                    moves.push(Move { from: from_sq, to: to_sq, promotion: Some(piece), is_castling: false, is_en_passant: false });
                }
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
            pushes &= pushes - 1;
        }

        // Avanço duplo
        let double_push = ((single_push & RANK_6) >> 8) & !all_pieces;
        let mut double_pushes = double_push;
        while double_pushes != 0 {
            let to_sq = double_pushes.trailing_zeros() as u8;
            moves.push(Move { from: to_sq + 16, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            double_pushes &= double_pushes - 1;
        }

        // Adiciona as capturas
        moves.extend(generate_pawn_captures(board));
    }
    moves
}

// =======================================================
// NOVA FUNÇÃO OTIMIZADA PARA A BUSCA DE QUIESCÊNCIA
// =======================================================

/// Gera apenas os lances de captura pseudo-legais para os peões.
pub fn generate_pawn_captures(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(8);

    if board.to_move == Color::White {
        let our_pawns = board.pawns & board.white_pieces;

        // Capturas para a direita
        let mut captures_right = ((our_pawns & NOT_H_FILE) << 9) & board.black_pieces;
        while captures_right != 0 {
            let to_sq = captures_right.trailing_zeros() as u8;
            let from_sq = to_sq - 9;
            if to_sq >= 56 { // Promoção
                for piece in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                    moves.push(Move { from: from_sq, to: to_sq, promotion: Some(piece), is_castling: false, is_en_passant: false });
                }
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
            captures_right &= captures_right - 1;
        }

        // Capturas para a esquerda
        let mut captures_left = ((our_pawns & NOT_A_FILE) << 7) & board.black_pieces;
        while captures_left != 0 {
            let to_sq = captures_left.trailing_zeros() as u8;
            let from_sq = to_sq - 7;
            if to_sq >= 56 { // Promoção
                for piece in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                    moves.push(Move { from: from_sq, to: to_sq, promotion: Some(piece), is_castling: false, is_en_passant: false });
                }
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
            captures_left &= captures_left - 1;
        }

        // En passant para brancas
        if let Some(ep_target) = board.en_passant_target {
            let ep_rank = ep_target / 8;
            if ep_rank == 5 {
                if ep_target % 8 > 0 {
                    let from_sq = ep_target - 9;
                    if from_sq / 8 == 4 {
                        if (our_pawns & (1u64 << from_sq)) != 0 {
                            moves.push(Move { from: from_sq, to: ep_target, promotion: None, is_castling: false, is_en_passant: true });
                        }
                    }
                }
                if ep_target % 8 < 7 {
                    let from_sq = ep_target - 7;
                    if from_sq / 8 == 4 {
                        if (our_pawns & (1u64 << from_sq)) != 0 {
                            moves.push(Move { from: from_sq, to: ep_target, promotion: None, is_castling: false, is_en_passant: true });
                        }
                    }
                }
            }
        }

    } else { // Lances das Pretas
        let our_pawns = board.pawns & board.black_pieces;

        // Capturas para a direita
        let mut captures_right = ((our_pawns & NOT_H_FILE) >> 7) & board.white_pieces;
        while captures_right != 0 {
            let to_sq = captures_right.trailing_zeros() as u8;
            let from_sq = to_sq + 7;
            if to_sq <= 7 { // Promoção
                for piece in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                    moves.push(Move { from: from_sq, to: to_sq, promotion: Some(piece), is_castling: false, is_en_passant: false });
                }
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
            captures_right &= captures_right - 1;
        }

        // Capturas para a esquerda
        let mut captures_left = ((our_pawns & NOT_A_FILE) >> 9) & board.white_pieces;
        while captures_left != 0 {
            let to_sq = captures_left.trailing_zeros() as u8;
            let from_sq = to_sq + 9;
            if to_sq <= 7 { // Promoção
                for piece in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                    moves.push(Move { from: from_sq, to: to_sq, promotion: Some(piece), is_castling: false, is_en_passant: false });
                }
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
            captures_left &= captures_left - 1;
        }

        // En passant para pretas
        if let Some(ep_target) = board.en_passant_target {
            let ep_rank = ep_target / 8;
            if ep_rank == 2 {
                if ep_target % 8 > 0 {
                    let from_sq = ep_target + 7;
                    if from_sq / 8 == 3 {
                        if (our_pawns & (1u64 << from_sq)) != 0 {
                            moves.push(Move { from: from_sq, to: ep_target, promotion: None, is_castling: false, is_en_passant: true });
                        }
                    }
                }
                if ep_target % 8 < 7 {
                    let from_sq = ep_target + 9;
                    if from_sq / 8 == 3 {
                        if (our_pawns & (1u64 << from_sq)) != 0 {
                            moves.push(Move { from: from_sq, to: ep_target, promotion: None, is_castling: false, is_en_passant: true });
                        }
                    }
                }
            }
        }
    }
    moves
}