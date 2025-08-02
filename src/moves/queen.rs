// Ficheiro: src/moves/queen.rs
// Descrição: Lógica para gerar os lances da Dama - OTIMIZADO COM MAGIC BITBOARDS.

use crate::{board::Board, types::{Move, Color, Bitboard}};
use super::magic_bitboards::get_queen_attacks_magic;

/// Gera todos os lances pseudo-legais para a dama do jogador atual (PERFORMANCE OTIMIZADA)
pub fn generate_queen_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(32);
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let all_pieces = board.white_pieces | board.black_pieces;
    let mut our_queens = board.queens & our_pieces;

    while our_queens != 0 {
        let from_sq = our_queens.trailing_zeros() as u8;
        our_queens &= our_queens - 1; // Remove LSB

        // Usa magic bitboards para calcular ataques instantaneamente
        let attacks = get_queen_attacks_magic(from_sq, all_pieces);
        
        // Remove ataques às nossas próprias peças
        let mut valid_attacks = attacks & !our_pieces;
        
        // Gera movimentos para todas as casas válidas
        while valid_attacks != 0 {
            let to_sq = valid_attacks.trailing_zeros() as u8;
            moves.push(Move {
                from: from_sq,
                to: to_sq,
                promotion: None,
                is_castling: false,
                is_en_passant: false,
            });
            valid_attacks &= valid_attacks - 1; // Remove LSB
        }
    }

    moves
}

/// Obtém o bitboard de ataques de rainha usando magic bitboards (ultra-rápido)
#[inline]
pub fn get_queen_attacks(square: u8, occupancy: Bitboard) -> Bitboard {
    super::magic_bitboards::get_queen_attacks_magic(square, occupancy)
}
