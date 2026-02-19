// Ficheiro: src/core/see.rs
// Descrição: Static Exchange Evaluation (SEE) para avaliar trocas de capturas.

use super::board::Board;
use super::types::*;
use crate::moves::magic_bitboards::{get_bishop_attacks_magic, get_rook_attacks_magic};
use crate::moves::knight::get_knight_attacks;
use crate::moves::king::get_king_attacks;

/// Piece values for SEE (simple, consistent ordering).
const SEE_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 20000];

#[inline]
fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => SEE_VALUES[0],
        PieceKind::Knight => SEE_VALUES[1],
        PieceKind::Bishop => SEE_VALUES[2],
        PieceKind::Rook => SEE_VALUES[3],
        PieceKind::Queen => SEE_VALUES[4],
        PieceKind::King => SEE_VALUES[5],
    }
}

impl Board {
    /// Static Exchange Evaluation: returns the estimated material gain/loss
    /// of a capture sequence starting with the given move.
    /// Positive = good for the side making the move.
    pub fn see(&self, mv: Move) -> i32 {
        let from = mv.from;
        let to = mv.to;

        // Determine the value of the initially captured piece
        let captured_value = if mv.is_en_passant {
            SEE_VALUES[0] // pawn
        } else if let Some(piece) = self.get_piece_at(to) {
            piece_value(piece.kind)
        } else {
            // Non-capture (shouldn't typically call SEE on non-captures)
            return 0;
        };

        // Attacker piece
        let attacker_piece = match self.get_piece_at(from) {
            Some(p) => p,
            None => return 0,
        };
        let attacker_value = if let Some(promo) = mv.promotion {
            piece_value(promo)
        } else {
            piece_value(attacker_piece.kind)
        };

        // Build gain array (swap list)
        let mut gain = [0i32; 32];
        let mut depth_idx = 0usize;

        gain[0] = captured_value;
        if mv.promotion.is_some() {
            gain[0] += piece_value(mv.promotion.unwrap()) - SEE_VALUES[0]; // promotion bonus
        }

        let mut occupancy = self.white_pieces | self.black_pieces;
        // Remove the initial attacker
        occupancy &= !(1u64 << from);

        let mut side = !attacker_piece.color; // next side to recapture
        let mut current_attacker_value = attacker_value;

        loop {
            depth_idx += 1;
            if depth_idx >= 32 { break; }

            gain[depth_idx] = current_attacker_value - gain[depth_idx - 1];

            // Pruning: if the side to move can't improve their position
            // (even capturing the piece won't help because opponent already ahead)
            if (-gain[depth_idx - 1]).max(gain[depth_idx]) < 0 {
                break;
            }

            // Find least valuable attacker of `side` to `to` square
            let (attacker_sq, attacker_kind) = match self.least_valuable_attacker(to, side, occupancy) {
                Some(result) => result,
                None => break, // no more attackers
            };

            // Remove this attacker from occupancy
            occupancy &= !(1u64 << attacker_sq);

            current_attacker_value = piece_value(attacker_kind);
            side = !side;
        }

        // Negamax the gain array
        while depth_idx > 1 {
            depth_idx -= 1;
            gain[depth_idx - 1] = -((-gain[depth_idx - 1]).max(gain[depth_idx]));
        }

        gain[0]
    }

    /// Returns true if SEE of the move is >= threshold.
    /// More efficient than computing full SEE when you just need a yes/no.
    #[inline]
    pub fn see_ge(&self, mv: Move, threshold: i32) -> bool {
        self.see(mv) >= threshold
    }

    /// Find the least valuable piece of `color` that attacks `square` given `occupancy`.
    /// Returns (square_of_attacker, piece_kind).
    fn least_valuable_attacker(&self, square: u8, color: Color, occupancy: Bitboard) -> Option<(u8, PieceKind)> {
        let color_pieces = if color == Color::White { self.white_pieces } else { self.black_pieces };
        let active = color_pieces & occupancy;

        // Pawns (cheapest)
        let pawn_attackers = crate::moves::pawn::get_pawn_attackers(square, !color);
        let pawns = pawn_attackers & self.pawns & active;
        if pawns != 0 {
            return Some((pawns.trailing_zeros() as u8, PieceKind::Pawn));
        }

        // Knights
        let knight_attacks = get_knight_attacks(square);
        let knights = knight_attacks & self.knights & active;
        if knights != 0 {
            return Some((knights.trailing_zeros() as u8, PieceKind::Knight));
        }

        // Bishops
        let bishop_attacks = get_bishop_attacks_magic(square, occupancy);
        let bishops = bishop_attacks & self.bishops & active;
        if bishops != 0 {
            return Some((bishops.trailing_zeros() as u8, PieceKind::Bishop));
        }

        // Rooks
        let rook_attacks = get_rook_attacks_magic(square, occupancy);
        let rooks = rook_attacks & self.rooks & active;
        if rooks != 0 {
            return Some((rooks.trailing_zeros() as u8, PieceKind::Rook));
        }

        // Queens (rook + bishop attacks)
        let queen_attacks = bishop_attacks | rook_attacks;
        let queens = queen_attacks & self.queens & active;
        if queens != 0 {
            return Some((queens.trailing_zeros() as u8, PieceKind::Queen));
        }

        // King
        let king_attacks = get_king_attacks(square);
        let kings = king_attacks & self.kings & active;
        if kings != 0 {
            return Some((kings.trailing_zeros() as u8, PieceKind::King));
        }

        None
    }
}
