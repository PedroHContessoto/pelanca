// Ficheiro: src/moves/sliding.rs
// Descrição: Lógica para gerar os lances de peças deslizantes (Torres e Bispos).

use crate::{board::Board, types::{Move, Color, PieceKind, Bitboard}};

// Placeholder for future magic bitboard optimization
// static BISHOP_MASKS: [Bitboard; 64] = generate_bishop_masks();
// static ROOK_MASKS: [Bitboard; 64] = generate_rook_masks();

/// Gera máscaras de ataque para bispos (exclui bordas) - Placeholder for magic bitboards
#[allow(dead_code)]
const fn generate_bishop_masks() -> [Bitboard; 64] {
    let mut masks = [0u64; 64];
    let mut square = 0;
    
    while square < 64 {
        let mut mask = 0u64;
        let rank = square as i32 / 8;
        let file = square as i32 % 8;
        
        // Diagonal principal (superior direita)
        let mut r = rank + 1;
        let mut f = file + 1;
        while r <= 6 && f <= 6 {
            mask |= 1u64 << (r * 8 + f);
            r += 1;
            f += 1;
        }
        
        // Diagonal principal (inferior esquerda)
        r = rank.saturating_sub(1);
        f = file.saturating_sub(1);
        while r >= 1 && f >= 1 && r < 8 && f < 8 {
            mask |= 1u64 << (r * 8 + f);
            if r == 0 || f == 0 { break; }
            r -= 1;
            f -= 1;
        }
        
        // Anti-diagonal (superior esquerda)
        r = rank + 1;
        f = file.saturating_sub(1);
        while r <= 6 && f >= 1 && f < 8 {
            mask |= 1u64 << (r * 8 + f);
            r += 1;
            if f == 0 { break; }
            f -= 1;
        }
        
        // Anti-diagonal (inferior direita)
        r = rank.saturating_sub(1);
        f = file + 1;
        while r >= 1 && f <= 6 && r < 8 {
            mask |= 1u64 << (r * 8 + f);
            if r == 0 { break; }
            r -= 1;
            f += 1;
        }
        
        masks[square] = mask;
        square += 1;
    }
    
    masks
}

/// Gera máscaras de ataque para torres (exclui bordas) - Placeholder for magic bitboards
#[allow(dead_code)]
const fn generate_rook_masks() -> [Bitboard; 64] {
    let mut masks = [0u64; 64];
    let mut square = 0;
    
    while square < 64 {
        let mut mask = 0u64;
        let rank = square as i32 / 8;
        let file = square as i32 % 8;
        
        // Horizontal direita
        let mut f = file + 1;
        while f <= 6 {
            mask |= 1u64 << (rank * 8 + f);
            f += 1;
        }
        
        // Horizontal esquerda
        f = file.saturating_sub(1);
        while f >= 1 && f < 8 {
            mask |= 1u64 << (rank * 8 + f);
            if f == 0 { break; }
            f -= 1;
        }
        
        // Vertical cima
        let mut r = rank + 1;
        while r <= 6 {
            mask |= 1u64 << (r * 8 + file);
            r += 1;
        }
        
        // Vertical baixo
        r = rank.saturating_sub(1);
        while r >= 1 && r < 8 {
            mask |= 1u64 << (r * 8 + file);
            if r == 0 { break; }
            r -= 1;
        }
        
        masks[square] = mask;
        square += 1;
    }
    
    masks
}

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

/// Gera lances ao longo de um raio a partir de uma casa numa dada direção.
/// É `pub(crate)` para que possa ser usado por `queen.rs`.
pub(crate) fn generate_ray_moves(moves: &mut Vec<Move>, board: &Board, from_sq: u8, direction: i8) {
    let our_pieces = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
    let enemy_pieces = if board.to_move == Color::White { board.black_pieces } else { board.white_pieces };
    let mut current_sq = from_sq as i8;

    loop {
        let prev_sq = current_sq;
        current_sq += direction;

        // Verificação 1: Saiu do tabuleiro (índice inválido).
        if !(0..64).contains(&current_sq) {
            break;
        }

        // Verificação 2: Deu a volta no tabuleiro (wrap-around).
        // A distância em colunas entre a casa anterior e a atual nunca pode ser maior que 1.
        // Isto previne saltos como de h1 para a2.
        let prev_file = prev_sq % 8;
        let current_file = current_sq % 8;
        if (current_file - prev_file).abs() > 1 {
            break;
        }

        let target_bb = 1u64 << current_sq;

        // Se a casa de destino contém uma peça nossa, paramos a busca nesta direção.
        if (target_bb & our_pieces) != 0 {
            break;
        }

        // Adiciona o movimento (seja um avanço para casa vazia ou uma captura).
        moves.push(Move { from: from_sq, to: current_sq as u8, promotion: None, is_castling: false, is_en_passant: false });

        // Se a casa de destino contém uma peça inimiga, adicionamos o lance de captura
        // e depois paramos a busca, pois não podemos saltar sobre ela.
        if (target_bb & enemy_pieces) != 0 {
            break;
        }
    }
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
