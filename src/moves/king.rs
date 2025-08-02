// Ficheiro: src/moves/king.rs
// Descrição: Lógica para gerar os lances do Rei.

use crate::{board::Board, types::{Move, Color, Bitboard}};

/// Tabela pré-computada de ataques de rei para máxima performance (1 ciclo CPU)
/// Cada posição contém o bitboard de ataques possíveis do rei naquela casa
static KING_ATTACKS: [Bitboard; 64] = [
    0x0000000000000302, 0x0000000000000705, 0x0000000000000e0a, 0x0000000000001c14,
    0x0000000000003828, 0x0000000000007050, 0x000000000000e0a0, 0x000000000000c040,
    0x0000000000030203, 0x0000000000070507, 0x00000000000e0a0e, 0x00000000001c141c,
    0x0000000000382838, 0x0000000000705070, 0x0000000000e0a0e0, 0x0000000000c040c0,
    0x0000000003020300, 0x0000000007050700, 0x000000000e0a0e00, 0x000000001c141c00,
    0x0000000038283800, 0x0000000070507000, 0x00000000e0a0e000, 0x00000000c040c000,
    0x0000000302030000, 0x0000000705070000, 0x0000000e0a0e0000, 0x0000001c141c0000,
    0x0000003828380000, 0x0000007050700000, 0x000000e0a0e00000, 0x000000c040c00000,
    0x0000030203000000, 0x0000070507000000, 0x00000e0a0e000000, 0x00001c141c000000,
    0x0000382838000000, 0x0000705070000000, 0x0000e0a0e0000000, 0x0000c040c0000000,
    0x0003020300000000, 0x0007050700000000, 0x000e0a0e00000000, 0x001c141c00000000,
    0x0038283800000000, 0x0070507000000000, 0x00e0a0e000000000, 0x00c040c000000000,
    0x0302030000000000, 0x0705070000000000, 0x0e0a0e0000000000, 0x1c141c0000000000,
    0x3828380000000000, 0x7050700000000000, 0xe0a0e00000000000, 0xc040c00000000000,
    0x0203000000000000, 0x0507000000000000, 0x0a0e000000000000, 0x141c000000000000,
    0x2838000000000000, 0x5070000000000000, 0xa0e0000000000000, 0x40c0000000000000,
];

/// Obtém o bitboard de ataque para um rei numa dada casa (ultra-rápido: 1 ciclo CPU)
#[inline]
pub fn get_king_attacks(square: u8) -> Bitboard {
    KING_ATTACKS[square as usize]
}

/// Gera todos os lances pseudo-legais para o rei usando tabela pré-computada (ULTRA RÁPIDO)
pub fn generate_king_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(8);
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let our_king = board.kings & our_pieces;

    if our_king == 0 { return moves; } // Não há rei no tabuleiro

    let from_sq = our_king.trailing_zeros() as u8;
    
    // Usa tabela pré-computada diretamente (1 ciclo CPU)
    let mut valid_moves = KING_ATTACKS[from_sq as usize] & !our_pieces;

    while valid_moves != 0 {
        let to_sq = valid_moves.trailing_zeros() as u8;
        valid_moves &= valid_moves - 1; // Remove LSB
        moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
    }

    // Lógica de roque com validação completa
    if board.to_move == Color::White {
        // Roque pequeno das brancas (e1-g1)
        if (board.castling_rights & 0b0001) != 0 {
            // Verifica se f1 e g1 estão vazias
            if (board.white_pieces | board.black_pieces) & 0b01100000 == 0 {
                // Verifica se rei não está em xeque e não passa por casas atacadas
                if !board.is_king_in_check(Color::White) && // rei não em xeque
                   !board.is_square_attacked_by(5, Color::Black) && // f1 não atacada
                   !board.is_square_attacked_by(6, Color::Black) {  // g1 não atacada
                    moves.push(Move { from: 4, to: 6, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
        
        // Roque grande das brancas (e1-c1)
        if (board.castling_rights & 0b0010) != 0 {
            // Verifica se b1, c1, d1 estão vazias
            if (board.white_pieces | board.black_pieces) & 0b00001110 == 0 {
                // Verifica se rei não está em xeque e não passa por casas atacadas
                if !board.is_king_in_check(Color::White) && // rei não em xeque
                   !board.is_square_attacked_by(3, Color::Black) && // d1 não atacada
                   !board.is_square_attacked_by(2, Color::Black) {  // c1 não atacada
                    moves.push(Move { from: 4, to: 2, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
    } else {
        // Roque pequeno das pretas (e8-g8)
        if (board.castling_rights & 0b0100) != 0 {
            // Verifica se f8 e g8 estão vazias
            if (board.white_pieces | board.black_pieces) & 0x6000000000000000 == 0 {
                // Verifica se rei não está em xeque e não passa por casas atacadas
                if !board.is_king_in_check(Color::Black) && // rei não em xeque
                   !board.is_square_attacked_by(61, Color::White) && // f8 não atacada
                   !board.is_square_attacked_by(62, Color::White) {  // g8 não atacada
                    moves.push(Move { from: 60, to: 62, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
        
        // Roque grande das pretas (e8-c8)
        if (board.castling_rights & 0b1000) != 0 {
            // Verifica se b8, c8, d8 estão vazias
            if (board.white_pieces | board.black_pieces) & 0x0e00000000000000 == 0 {
                // Verifica se rei não está em xeque e não passa por casas atacadas
                if !board.is_king_in_check(Color::Black) && // rei não em xeque
                   !board.is_square_attacked_by(59, Color::White) && // d8 não atacada
                   !board.is_square_attacked_by(58, Color::White) {  // c8 não atacada
                    moves.push(Move { from: 60, to: 58, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
    }

    moves
}
