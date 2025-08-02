// Ficheiro: src/moves/queen.rs
// Descrição: Lógica para gerar os lances da Dama - OTIMIZADO COM MAGIC BITBOARDS.

use crate::{board::Board, types::{Move, Color}};
use super::magic_bitboards::get_queen_attacks_magic;

/// Gera todos os lances pseudo-legais para a dama do jogador atual (PERFORMANCE OTIMIZADA)
pub fn generate_queen_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(32);
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let enemy_pieces = if board.to_move == Color::White { board.black_pieces } else { board.white_pieces };
    let all_pieces = board.white_pieces | board.black_pieces;
    let mut our_queens = board.queens & our_pieces;

    while our_queens != 0 {
        let from_sq = crate::intrinsics::trailing_zeros(our_queens) as u8;
        our_queens = crate::intrinsics::reset_lsb(our_queens);

        // Usa magic bitboards para calcular ataques instantaneamente
        let attacks = get_queen_attacks_magic(from_sq, all_pieces);
        
        // Remove ataques às nossas próprias peças
        let valid_attacks = attacks & !our_pieces;
        
        // Gera movimentos para todas as casas válidas usando intrinsics otimizados
        for to_sq in crate::intrinsics::BitboardOps::iter_squares(valid_attacks) {
            moves.push(Move {
                from: from_sq,
                to: to_sq,
                promotion: None,
                is_castling: false,
                is_en_passant: false,
            });
        }
    }

    moves
}
