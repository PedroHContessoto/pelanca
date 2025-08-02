// Ficheiro: src/moves/pawn.rs
// Descrição: Lógica para gerar os lances dos peões - OTIMIZADO COM TABELAS PRÉ-COMPUTADAS.

use crate::{board::Board, types::{Move, Color, Bitboard, PieceKind}};

// Constantes importadas ou redefinidas para este módulo
const NOT_A_FILE: Bitboard = 0xfefefefefefefefe;
const NOT_H_FILE: Bitboard = 0x7f7f7f7f7f7f7f7f;
const RANK_3: Bitboard = 0x0000000000FF0000;
const RANK_6: Bitboard = 0x0000FF0000000000;

// ============================================================================
// TABELAS PRÉ-COMPUTADAS PARA MÁXIMA PERFORMANCE
// ============================================================================

// Tabelas de ataques de peão
static WHITE_PAWN_ATTACKS: [Bitboard; 64] = [
    0x0000000000000200, 0x0000000000000500, 0x0000000000000a00, 0x0000000000001400,
    0x0000000000002800, 0x0000000000005000, 0x000000000000a000, 0x0000000000004000,
    0x0000000000020000, 0x0000000000050000, 0x00000000000a0000, 0x0000000000140000,
    0x0000000000280000, 0x0000000000500000, 0x0000000000a00000, 0x0000000000400000,
    0x0000000002000000, 0x0000000005000000, 0x000000000a000000, 0x0000000014000000,
    0x0000000028000000, 0x0000000050000000, 0x00000000a0000000, 0x0000000040000000,
    0x0000000200000000, 0x0000000500000000, 0x0000000a00000000, 0x0000001400000000,
    0x0000002800000000, 0x0000005000000000, 0x000000a000000000, 0x0000004000000000,
    0x0000020000000000, 0x0000050000000000, 0x00000a0000000000, 0x0000140000000000,
    0x0000280000000000, 0x0000500000000000, 0x0000a00000000000, 0x0000400000000000,
    0x0002000000000000, 0x0005000000000000, 0x000a000000000000, 0x0014000000000000,
    0x0028000000000000, 0x0050000000000000, 0x00a0000000000000, 0x0040000000000000,
    0x0200000000000000, 0x0500000000000000, 0x0a00000000000000, 0x1400000000000000,
    0x2800000000000000, 0x5000000000000000, 0xa000000000000000, 0x4000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
];

static BLACK_PAWN_ATTACKS: [Bitboard; 64] = [
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000002, 0x0000000000000005, 0x000000000000000a, 0x0000000000000014,
    0x0000000000000028, 0x0000000000000050, 0x00000000000000a0, 0x0000000000000040,
    0x0000000000000200, 0x0000000000000500, 0x0000000000000a00, 0x0000000000001400,
    0x0000000000002800, 0x0000000000005000, 0x000000000000a000, 0x0000000000004000,
    0x0000000000020000, 0x0000000000050000, 0x00000000000a0000, 0x0000000000140000,
    0x0000000000280000, 0x0000000000500000, 0x0000000000a00000, 0x0000000000400000,
    0x0000000002000000, 0x0000000005000000, 0x000000000a000000, 0x0000000014000000,
    0x0000000028000000, 0x0000000050000000, 0x00000000a0000000, 0x0000000040000000,
    0x0000000200000000, 0x0000000500000000, 0x0000000a00000000, 0x0000001400000000,
    0x0000002800000000, 0x0000005000000000, 0x000000a000000000, 0x0000004000000000,
    0x0000020000000000, 0x0000050000000000, 0x00000a0000000000, 0x0000140000000000,
    0x0000280000000000, 0x0000500000000000, 0x0000a00000000000, 0x0000400000000000,
    0x0002000000000000, 0x0005000000000000, 0x000a000000000000, 0x0014000000000000,
    0x0028000000000000, 0x0050000000000000, 0x00a0000000000000, 0x0040000000000000,
];

// Tabelas de movimentos de peão (avanços simples)
static WHITE_PAWN_MOVES: [Bitboard; 64] = [
    0x0000000000000100, 0x0000000000000200, 0x0000000000000400, 0x0000000000000800,
    0x0000000000001000, 0x0000000000002000, 0x0000000000004000, 0x0000000000008000,
    0x0000000000010000, 0x0000000000020000, 0x0000000000040000, 0x0000000000080000,
    0x0000000000100000, 0x0000000000200000, 0x0000000000400000, 0x0000000000800000,
    0x0000000001000000, 0x0000000002000000, 0x0000000004000000, 0x0000000008000000,
    0x0000000010000000, 0x0000000020000000, 0x0000000040000000, 0x0000000080000000,
    0x0000000100000000, 0x0000000200000000, 0x0000000400000000, 0x0000000800000000,
    0x0000001000000000, 0x0000002000000000, 0x0000004000000000, 0x0000008000000000,
    0x0000010000000000, 0x0000020000000000, 0x0000040000000000, 0x0000080000000000,
    0x0000100000000000, 0x0000200000000000, 0x0000400000000000, 0x0000800000000000,
    0x0001000000000000, 0x0002000000000000, 0x0004000000000000, 0x0008000000000000,
    0x0010000000000000, 0x0020000000000000, 0x0040000000000000, 0x0080000000000000,
    0x0100000000000000, 0x0200000000000000, 0x0400000000000000, 0x0800000000000000,
    0x1000000000000000, 0x2000000000000000, 0x4000000000000000, 0x8000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
];

static BLACK_PAWN_MOVES: [Bitboard; 64] = [
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000001, 0x0000000000000002, 0x0000000000000004, 0x0000000000000008,
    0x0000000000000010, 0x0000000000000020, 0x0000000000000040, 0x0000000000000080,
    0x0000000000000100, 0x0000000000000200, 0x0000000000000400, 0x0000000000000800,
    0x0000000000001000, 0x0000000000002000, 0x0000000000004000, 0x0000000000008000,
    0x0000000000010000, 0x0000000000020000, 0x0000000000040000, 0x0000000000080000,
    0x0000000000100000, 0x0000000000200000, 0x0000000000400000, 0x0000000000800000,
    0x0000000001000000, 0x0000000002000000, 0x0000000004000000, 0x0000000008000000,
    0x0000000010000000, 0x0000000020000000, 0x0000000040000000, 0x0000000080000000,
    0x0000000100000000, 0x0000000200000000, 0x0000000400000000, 0x0000000800000000,
    0x0000001000000000, 0x0000002000000000, 0x0000004000000000, 0x0000008000000000,
    0x0000010000000000, 0x0000020000000000, 0x0000040000000000, 0x0000080000000000,
    0x0000100000000000, 0x0000200000000000, 0x0000400000000000, 0x0000800000000000,
    0x0001000000000000, 0x0002000000000000, 0x0004000000000000, 0x0008000000000000,
    0x0010000000000000, 0x0020000000000000, 0x0040000000000000, 0x0080000000000000,
];

// Tabelas de movimentos duplos de peão (da linha inicial)
static WHITE_PAWN_DOUBLE_MOVES: [Bitboard; 64] = [
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000001000000, 0x0000000002000000, 0x0000000004000000, 0x0000000008000000,
    0x0000000010000000, 0x0000000020000000, 0x0000000040000000, 0x0000000080000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
];

static BLACK_PAWN_DOUBLE_MOVES: [Bitboard; 64] = [
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000010000, 0x0000000000020000, 0x0000000000040000, 0x0000000000080000,
    0x0000000000100000, 0x0000000000200000, 0x0000000000400000, 0x0000000000800000,
];

/// Obtém o bitboard de ataques de peão usando tabela pré-computada (ULTRA RÁPIDO)
#[inline(always)]
pub fn get_pawn_attacks(square: u8, color: Color) -> Bitboard {
    match color {
        Color::White => WHITE_PAWN_ATTACKS[square as usize],
        Color::Black => BLACK_PAWN_ATTACKS[square as usize],
    }
}

/// Obtém o bitboard de movimentos simples de peão usando tabela pré-computada
#[inline(always)]
pub fn get_pawn_moves(square: u8, color: Color) -> Bitboard {
    match color {
        Color::White => WHITE_PAWN_MOVES[square as usize],
        Color::Black => BLACK_PAWN_MOVES[square as usize],
    }
}

/// Obtém o bitboard de movimentos duplos de peão usando tabela pré-computada
#[inline(always)]
pub fn get_pawn_double_moves(square: u8, color: Color) -> Bitboard {
    match color {
        Color::White => WHITE_PAWN_DOUBLE_MOVES[square as usize],
        Color::Black => BLACK_PAWN_DOUBLE_MOVES[square as usize],
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