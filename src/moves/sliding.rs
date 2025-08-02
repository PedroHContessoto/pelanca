// Ficheiro: src/moves/sliding.rs
// Descrição: Lógica para gerar os lances de peças deslizantes (Torres e Bispos).

use crate::{board::Board, types::{Move, Color, PieceKind, Bitboard}};

// Placeholder for future magic bitboard optimization
// static BISHOP_MASKS: [Bitboard; 64] = generate_bishop_masks();
// static ROOK_MASKS: [Bitboard; 64] = generate_rook_masks();


/// Calcula ataques de bispo usando Magic Bitboards (PERFORMANCE CRÍTICA)
pub fn get_bishop_attacks(square: u8, occupancy: Bitboard) -> Bitboard {
    use super::magic_bitboards::get_bishop_attacks_magic;
    get_bishop_attacks_magic(square, occupancy)
}

/// Calcula ataques de torre usando Magic Bitboards (PERFORMANCE CRÍTICA)
pub fn get_rook_attacks(square: u8, occupancy: Bitboard) -> Bitboard {
    use super::magic_bitboards::get_rook_attacks_magic;
    get_rook_attacks_magic(square, occupancy)
}

/// Função genérica otimizada para gerar lances de Torres e Bispos.
pub fn generate_sliding_moves(board: &Board, piece_kind: PieceKind) -> Vec<Move> {
    let mut moves = Vec::with_capacity(32);
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let all_pieces = board.white_pieces | board.black_pieces;

    let piece_bb = match piece_kind {
        PieceKind::Bishop => board.bishops,
        PieceKind::Rook => board.rooks,
        _ => 0 // Não deve acontecer para esta função
    };

    let mut our_sliding_pieces = piece_bb & our_pieces;

    while our_sliding_pieces != 0 {
        let from_sq = our_sliding_pieces.trailing_zeros() as u8;
        
        // Usa funções otimizadas de ataque
        let attacks = if piece_kind == PieceKind::Bishop {
            get_bishop_attacks(from_sq, all_pieces)
        } else {
            get_rook_attacks(from_sq, all_pieces)
        };
        
        // Filtra movimentos válidos (exclui nossas próprias peças)
        let mut valid_moves = attacks & !our_pieces;
        
        while valid_moves != 0 {
            let to_sq = valid_moves.trailing_zeros() as u8;
            moves.push(Move { 
                from: from_sq, 
                to: to_sq, 
                promotion: None, 
                is_castling: false, 
                is_en_passant: false 
            });
            valid_moves &= valid_moves - 1;
        }
        
        our_sliding_pieces &= our_sliding_pieces - 1;
    }
    moves
}
