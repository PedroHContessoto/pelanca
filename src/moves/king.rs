// Ficheiro: src/moves/king.rs
// Descrição: Lógica para gerar os lances do Rei.

use crate::{board::Board, types::{Move, Color, Bitboard}};

/// Gera a tabela de ataques de rei para todas as 64 casas.
const fn generate_king_attacks_table() -> [Bitboard; 64] {
    let mut attacks = [0u64; 64];
    let mut square = 0;
    
    while square < 64 {
        let king_pos = 1u64 << square;
        let mut attack_bb = 0u64;
        let s = square as i8;

        // Movimentos de um passo em todas as 8 direções.
        if s % 8 > 0 { attack_bb |= king_pos >> 1; } // Esquerda
        if s % 8 < 7 { attack_bb |= king_pos << 1; } // Direita
        if s / 8 > 0 { attack_bb |= king_pos >> 8; } // Baixo
        if s / 8 < 7 { attack_bb |= king_pos << 8; } // Cima
        if s % 8 > 0 && s / 8 > 0 { attack_bb |= king_pos >> 9; } // Baixo-Esquerda
        if s % 8 < 7 && s / 8 > 0 { attack_bb |= king_pos >> 7; } // Baixo-Direita
        if s % 8 > 0 && s / 8 < 7 { attack_bb |= king_pos << 7; } // Cima-Esquerda
        if s % 8 < 7 && s / 8 < 7 { attack_bb |= king_pos << 9; } // Cima-Direita

        attacks[square] = attack_bb;
        square += 1;
    }
    
    attacks
}

/// Tabela pré-calculada de ataques de rei para cada casa do tabuleiro.
static KING_ATTACKS: [Bitboard; 64] = generate_king_attacks_table();

/// Obtém o bitboard de ataque para um rei numa dada casa usando lookup table.
#[inline]
pub fn get_king_attacks_lookup(square: u8) -> Bitboard {
    KING_ATTACKS[square as usize]
}

/// Calcula o bitboard de ataque para um rei numa dada casa.
#[inline]
fn get_king_attacks(square: u8) -> Bitboard {
    KING_ATTACKS[square as usize]
}

/// Gera todos os lances pseudo-legais para o rei do jogador atual.
pub fn generate_king_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(8); // Pre-aloca para reduzir realocações
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let our_king = board.kings & our_pieces;

    if our_king == 0 { return moves; } // Não há rei no tabuleiro (impossível em jogo normal)

    let from_sq = our_king.trailing_zeros() as u8;
    let attacks = get_king_attacks(from_sq);
    let mut valid_moves = attacks & !our_pieces;

    while valid_moves != 0 {
        let to_sq = valid_moves.trailing_zeros() as u8;
        moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
        valid_moves &= valid_moves - 1;
    }

    // Lógica de roque
    if board.to_move == Color::White {
        // Roque pequeno das brancas (e1-g1)
        if (board.castling_rights & 0b0001) != 0 {
            if (board.white_pieces | board.black_pieces) & 0b01100000 == 0 { // f1 e g1 vazias
                if !board.is_square_attacked_by(4, Color::Black) && // e1 não atacada
                   !board.is_square_attacked_by(5, Color::Black) && // f1 não atacada
                   !board.is_square_attacked_by(6, Color::Black) {  // g1 não atacada
                    moves.push(Move { from: 4, to: 6, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
        
        // Roque grande das brancas (e1-c1)
        if (board.castling_rights & 0b0010) != 0 {
            if (board.white_pieces | board.black_pieces) & 0b00001110 == 0 { // b1, c1, d1 vazias
                if !board.is_square_attacked_by(4, Color::Black) && // e1 não atacada
                   !board.is_square_attacked_by(3, Color::Black) && // d1 não atacada
                   !board.is_square_attacked_by(2, Color::Black) {  // c1 não atacada
                    moves.push(Move { from: 4, to: 2, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
    } else {
        // Roque pequeno das pretas (e8-g8)
        if (board.castling_rights & 0b0100) != 0 {
            if (board.white_pieces | board.black_pieces) & 0x6000000000000000 == 0 { // f8 e g8 vazias
                if !board.is_square_attacked_by(60, Color::White) && // e8 não atacada
                   !board.is_square_attacked_by(61, Color::White) && // f8 não atacada
                   !board.is_square_attacked_by(62, Color::White) {  // g8 não atacada
                    moves.push(Move { from: 60, to: 62, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
        
        // Roque grande das pretas (e8-c8)
        if (board.castling_rights & 0b1000) != 0 {
            if (board.white_pieces | board.black_pieces) & 0x0e00000000000000 == 0 { // b8, c8, d8 vazias
                if !board.is_square_attacked_by(60, Color::White) && // e8 não atacada
                   !board.is_square_attacked_by(59, Color::White) && // d8 não atacada
                   !board.is_square_attacked_by(58, Color::White) {  // c8 não atacada
                    moves.push(Move { from: 60, to: 58, promotion: None, is_castling: true, is_en_passant: false });
                }
            }
        }
    }

    moves
}
