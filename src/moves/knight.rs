// Ficheiro: src/moves/knight.rs
// Descrição: Lógica para gerar os lances dos cavalos.

use crate::{board::Board, types::{Move, Color, Bitboard}};

/// Gera a tabela de ataques de cavalo para todas as 64 casas.
const fn generate_knight_attacks_table() -> [Bitboard; 64] {
    let mut attacks = [0u64; 64];
    let mut square = 0;
    
    while square < 64 {
        let mut attack_bb = 0u64;
        let s = square as i8;
        
        // Array com os possíveis deslocamentos do cavalo
        let knight_moves = [15, 17, 6, 10, -15, -17, -6, -10];
        let mut i = 0;
        
        while i < knight_moves.len() {
            let offset = knight_moves[i];
            let target = s + offset;
            
            // Verifica se o movimento está dentro do tabuleiro
            if target >= 0 && target < 64 {
                // Verifica se houve wrap-around horizontal
                let from_file = s % 8;
                let to_file = target % 8;
                let file_diff = if to_file > from_file { 
                    to_file - from_file 
                } else { 
                    from_file - to_file 
                };
                
                // Um movimento de cavalo válido deve ter diferença de coluna de 1 ou 2
                if file_diff <= 2 {
                    attack_bb |= 1u64 << target;
                }
            }
            i += 1;
        }
        
        attacks[square] = attack_bb;
        square += 1;
    }
    
    attacks
}

/// Tabela pré-calculada de ataques de cavalo para cada casa do tabuleiro.
static KNIGHT_ATTACKS: [Bitboard; 64] = generate_knight_attacks_table();

/// Obtém o bitboard de ataque para um cavalo numa dada casa usando lookup table.
#[inline]
pub fn get_knight_attacks_lookup(square: u8) -> Bitboard {
    KNIGHT_ATTACKS[square as usize]
}

/// Obtém o bitboard de ataque para um cavalo numa dada casa usando lookup table.
#[inline]
fn get_knight_attacks(square: u8) -> Bitboard {
    KNIGHT_ATTACKS[square as usize]
}

/// Gera todos os lances pseudo-legais para os cavalos do jogador atual.
pub fn generate_knight_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(16); // Pre-aloca para reduzir realocações
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let mut our_knights = board.knights & our_pieces;

    while our_knights != 0 {
        let from_sq = our_knights.trailing_zeros() as u8;
        let attacks = get_knight_attacks(from_sq);
        let mut valid_moves = attacks & !our_pieces;

        while valid_moves != 0 {
            let to_sq = valid_moves.trailing_zeros() as u8;
            moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            valid_moves &= valid_moves - 1;
        }
        our_knights &= our_knights - 1;
    }
    moves
}
