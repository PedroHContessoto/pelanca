use crate::core::*;
use crate::utils::*;

/// Avaliação ultra-rápida otimizada para velocidade máxima
#[inline(always)]
pub fn evaluate_position(board: &Board) -> i32 {
    // Usa intrinsics para contagem ultra-rápida de bits
    let white_pawns = popcount(board.white_pieces & board.pawns);
    let black_pawns = popcount(board.black_pieces & board.pawns);
    let white_knights = popcount(board.white_pieces & board.knights);
    let black_knights = popcount(board.black_pieces & board.knights);
    let white_bishops = popcount(board.white_pieces & board.bishops);
    let black_bishops = popcount(board.black_pieces & board.bishops);
    let white_rooks = popcount(board.white_pieces & board.rooks);
    let black_rooks = popcount(board.black_pieces & board.rooks);
    let white_queens = popcount(board.white_pieces & board.queens);
    let black_queens = popcount(board.black_pieces & board.queens);
    
    // Material bruto (ultra-rápido)
    let material_score = 
        (white_pawns as i32 - black_pawns as i32) * 100 +
        (white_knights as i32 - black_knights as i32) * 320 +
        (white_bishops as i32 - black_bishops as i32) * 330 +
        (white_rooks as i32 - black_rooks as i32) * 500 +
        (white_queens as i32 - black_queens as i32) * 900;
    
    // Bônus posicional super-rápido usando bitboards
    let mut positional_score = 0;
    
    // Controle do centro (e4, e5, d4, d5)
    const CENTER: u64 = 0x0000001818000000;
    let white_center = popcount(board.white_pieces & CENTER);
    let black_center = popcount(board.black_pieces & CENTER);
    positional_score += (white_center as i32 - black_center as i32) * 30;
    
    // Desenvolvimento de peças (saíram das casas iniciais)
    const WHITE_BACK_RANK: u64 = 0x00000000000000FF;
    const BLACK_BACK_RANK: u64 = 0xFF00000000000000;
    let white_developed = popcount((board.white_pieces & (board.knights | board.bishops)) & !WHITE_BACK_RANK);
    let black_developed = popcount((board.black_pieces & (board.knights | board.bishops)) & !BLACK_BACK_RANK);
    positional_score += (white_developed as i32 - black_developed as i32) * 25;
    
    // Peões passados (muito simples mas rápido)
    let white_passed = count_passed_pawns_fast(board, Color::White);
    let black_passed = count_passed_pawns_fast(board, Color::Black);
    positional_score += (white_passed - black_passed) * 50;
    
    // Penalidade por rei em xeque (cache do board)
    if board.white_king_in_check { positional_score -= 40; }
    if board.black_king_in_check { positional_score += 40; }
    
    material_score + positional_score
}

/// Conta peões passados de forma ultra-rápida
#[inline(always)]
fn count_passed_pawns_fast(board: &Board, color: Color) -> i32 {
    let (my_pawns, enemy_pawns) = if color == Color::White {
        (board.white_pieces & board.pawns, board.black_pieces & board.pawns)
    } else {
        (board.black_pieces & board.pawns, board.white_pieces & board.pawns)
    };
    
    let mut passed_count = 0;
    let mut pawns = my_pawns;
    
    // Itera pelos peões usando intrinsics ultra-rápidos
    while pawns != 0 {
        let square = trailing_zeros(pawns) as u8;
        pawns = reset_lsb(pawns);
        
        let file = square % 8;
        let rank = square / 8;
        
        // Máscara de arquivos adjacentes
        let mut file_mask = 0x0101010101010101u64 << file;
        if file > 0 { file_mask |= 0x0101010101010101u64 << (file - 1); }
        if file < 7 { file_mask |= 0x0101010101010101u64 << (file + 1); }
        
        // Máscara à frente
        let front_mask = if color == Color::White {
            !((1u64 << ((rank + 1) * 8)) - 1)
        } else {
            (1u64 << (rank * 8)) - 1
        };
        
        // Se não há peões inimigos à frente, é passado
        if (enemy_pawns & file_mask & front_mask) == 0 {
            passed_count += 1;
        }
    }
    
    passed_count
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