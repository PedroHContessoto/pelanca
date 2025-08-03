use crate::core::*;

/// Avaliação simples baseada no material das peças
pub fn evaluate_position(board: &Board) -> i32 {
    let mut score = 0;
    
    // Valores das peças
    const PAWN_VALUE: i32 = 100;
    const KNIGHT_VALUE: i32 = 320;
    const BISHOP_VALUE: i32 = 330;
    const ROOK_VALUE: i32 = 500;
    const QUEEN_VALUE: i32 = 900;
    
    // Conta material das brancas
    score += (board.white_pieces & board.pawns).count_ones() as i32 * PAWN_VALUE;
    score += (board.white_pieces & board.knights).count_ones() as i32 * KNIGHT_VALUE;
    score += (board.white_pieces & board.bishops).count_ones() as i32 * BISHOP_VALUE;
    score += (board.white_pieces & board.rooks).count_ones() as i32 * ROOK_VALUE;
    score += (board.white_pieces & board.queens).count_ones() as i32 * QUEEN_VALUE;
    
    // Subtrai material das pretas
    score -= (board.black_pieces & board.pawns).count_ones() as i32 * PAWN_VALUE;
    score -= (board.black_pieces & board.knights).count_ones() as i32 * KNIGHT_VALUE;
    score -= (board.black_pieces & board.bishops).count_ones() as i32 * BISHOP_VALUE;
    score -= (board.black_pieces & board.rooks).count_ones() as i32 * ROOK_VALUE;
    score -= (board.black_pieces & board.queens).count_ones() as i32 * QUEEN_VALUE;
    
    // Bônus de mobilidade simples
    let white_moves = count_legal_moves(board, Color::White);
    let black_moves = count_legal_moves(board, Color::Black);
    score += (white_moves as i32 - black_moves as i32) * 10;
    
    // Penalidade por rei em xeque
    if board.is_king_in_check(Color::White) {
        score -= 50;
    }
    if board.is_king_in_check(Color::Black) {
        score += 50;
    }
    
    // Retorna score do ponto de vista das brancas
    score
}

/// Conta número de movimentos legais para uma cor
fn count_legal_moves(board: &Board, color: Color) -> u32 {
    if board.to_move != color {
        // Precisa simular troca de turno
        let mut temp_board = *board;
        temp_board.to_move = color;
        temp_board.generate_legal_moves().len() as u32
    } else {
        board.generate_legal_moves().len() as u32
    }
}

/// Verifica se a posição é terminal (mate ou empate)
pub fn is_terminal_position(board: &Board) -> Option<i32> {
    if board.is_checkmate() {
        // Xeque-mate: -infinito para quem está em mate
        if board.to_move == Color::White {
            Some(-30000)
        } else {
            Some(30000)
        }
    } else if board.is_stalemate() || board.is_draw_by_insufficient_material() || board.is_draw_by_50_moves() {
        // Empate
        Some(0)
    } else {
        None
    }
}