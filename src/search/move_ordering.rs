use crate::core::*;
use crate::utils::*;

/// Ordena movimentos para maximizar podas Alpha-Beta
#[inline(always)]
pub fn order_moves(board: &Board, moves: &mut Vec<Move>) {
    // Ordena por score MVV-LVA + bonificações posicionais
    moves.sort_unstable_by_key(|mv| std::cmp::Reverse(score_move(board, *mv)));
}

/// Pontua movimento para ordenação (quanto maior, melhor)
#[inline(always)]
fn score_move(board: &Board, mv: Move) -> i32 {
    let mut score = 0;
    
    let from_bb = 1u64 << mv.from;
    let to_bb = 1u64 << mv.to;
    
    // Identifica peça que se move (ultra-rápido usando bitboards)
    let moving_piece = get_piece_type_fast(board, from_bb);
    
    // ========================================================================
    // MVV-LVA (Most Valuable Victim - Least Valuable Attacker)
    // ========================================================================
    let enemy_pieces = if board.to_move == Color::White { board.black_pieces } else { board.white_pieces };
    if (enemy_pieces & to_bb) != 0 {
        let victim_piece = get_piece_type_fast(board, to_bb);
        score += mvv_lva_score(victim_piece, moving_piece);
    }
    
    // ========================================================================
    // BONIFICAÇÕES POSICIONAIS (ultra-rápidas)
    // ========================================================================
    
    // Promoções (prioridade máxima)
    if let Some(promotion) = mv.promotion {
        score += match promotion {
            PieceKind::Queen => 9000,
            PieceKind::Rook => 5000,
            PieceKind::Bishop => 3300,
            PieceKind::Knight => 3200,
            _ => 0,
        };
    }
    
    // Movimentos para o centro
    const CENTER: u64 = 0x0000001818000000; // e4, e5, d4, d5
    const EXTENDED_CENTER: u64 = 0x00003C3C3C3C0000; // c3-f3 até c6-f6
    
    if (to_bb & CENTER) != 0 {
        score += 100;
    } else if (to_bb & EXTENDED_CENTER) != 0 {
        score += 50;
    }
    
    // Desenvolvimento de peças menores
    if moving_piece == PieceKind::Knight || moving_piece == PieceKind::Bishop {
        const WHITE_BACK_RANK: u64 = 0x00000000000000FF;
        const BLACK_BACK_RANK: u64 = 0xFF00000000000000;
        
        let back_rank = if board.to_move == Color::White { WHITE_BACK_RANK } else { BLACK_BACK_RANK };
        if (from_bb & back_rank) != 0 && (to_bb & back_rank) == 0 {
            score += 80; // Bônus por desenvolvimento
        }
    }
    
    // Roque (alta prioridade)
    if mv.is_castling {
        score += 200;
    }
    
    // Ataques ao rei inimigo (aproximação)
    let enemy_king = if board.to_move == Color::White {
        board.kings & board.black_pieces
    } else {
        board.kings & board.white_pieces
    };
    
    if enemy_king != 0 {
        let king_square = trailing_zeros(enemy_king) as u8;
        let distance_before = manhattan_distance(mv.from, king_square);
        let distance_after = manhattan_distance(mv.to, king_square);
        
        if distance_after < distance_before {
            score += 30; // Bônus por aproximar do rei
        }
    }
    
    // Penalty para mover peças já desenvolvidas múltiplas vezes
    if moving_piece == PieceKind::Knight || moving_piece == PieceKind::Bishop {
        const WHITE_BACK_RANK: u64 = 0x00000000000000FF;
        const BLACK_BACK_RANK: u64 = 0xFF00000000000000;
        
        let back_rank = if board.to_move == Color::White { WHITE_BACK_RANK } else { BLACK_BACK_RANK };
        if (from_bb & back_rank) == 0 {
            score -= 20; // Penalty por mover peça já desenvolvida
        }
    }
    
    score
}

/// MVV-LVA scoring table (ultra-otimizada)
#[inline(always)]
fn mvv_lva_score(victim: PieceKind, attacker: PieceKind) -> i32 {
    const MVV_LVA: [[i32; 6]; 6] = [
        // Vítima: Pawn, Knight, Bishop, Rook, Queen, King
        [105, 205, 305, 405, 505, 605], // Atacante: Pawn
        [104, 204, 304, 404, 504, 604], // Atacante: Knight  
        [103, 203, 303, 403, 503, 603], // Atacante: Bishop
        [102, 202, 302, 402, 502, 602], // Atacante: Rook
        [101, 201, 301, 401, 501, 601], // Atacante: Queen
        [100, 200, 300, 400, 500, 600], // Atacante: King
    ];
    
    MVV_LVA[piece_to_index(attacker)][piece_to_index(victim)]
}

/// Identifica tipo de peça ultra-rapidamente usando bitboards
#[inline(always)]
fn get_piece_type_fast(board: &Board, bb: u64) -> PieceKind {
    if (board.pawns & bb) != 0 { PieceKind::Pawn }
    else if (board.knights & bb) != 0 { PieceKind::Knight }
    else if (board.bishops & bb) != 0 { PieceKind::Bishop }
    else if (board.rooks & bb) != 0 { PieceKind::Rook }
    else if (board.queens & bb) != 0 { PieceKind::Queen }
    else { PieceKind::King }
}

/// Converte PieceKind para índice (inline para performance)
#[inline(always)]
fn piece_to_index(piece: PieceKind) -> usize {
    match piece {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    }
}

/// Distância Manhattan ultra-rápida
#[inline(always)]
fn manhattan_distance(from: u8, to: u8) -> u8 {
    let from_file = from % 8;
    let from_rank = from / 8;
    let to_file = to % 8;
    let to_rank = to / 8;
    
    ((from_file as i8 - to_file as i8).abs() + (from_rank as i8 - to_rank as i8).abs()) as u8
}