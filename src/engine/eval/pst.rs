// Ficheiro: src/engine/eval/pst.rs
// Descrição: Piece-Square Tables (PST) para avaliação posicional.
// Tabelas clássicas do ponto de vista das Brancas (rank 1 = índice 0..7).
// Para Pretas, espelhamos verticalmente.

use crate::core::board::Board;
use crate::core::types::Color;
use crate::engine::traits::{Evaluator, Score};

// ============================================================================
// Piece-Square Tables (perspectiva Brancas, a1=index 0, h8=index 63)
// Fonte: valores clássicos PeSTO/Simplified Eval Function
// ============================================================================

#[rustfmt::skip]
const PAWN_PST: [Score; 64] = [
     0,   0,   0,   0,   0,   0,   0,   0,
    50,  50,  50,  50,  50,  50,  50,  50,
    10,  10,  20,  30,  30,  20,  10,  10,
     5,   5,  10,  25,  25,  10,   5,   5,
     0,   0,   0,  20,  20,   0,   0,   0,
     5,  -5, -10,   0,   0, -10,  -5,   5,
     5,  10,  10, -20, -20,  10,  10,   5,
     0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const KNIGHT_PST: [Score; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,   0,   0,   0,   0, -20, -40,
    -30,   0,  10,  15,  15,  10,   0, -30,
    -30,   5,  15,  20,  20,  15,   5, -30,
    -30,   0,  15,  20,  20,  15,   0, -30,
    -30,   5,  10,  15,  15,  10,   5, -30,
    -40, -20,   0,   5,   5,   0, -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50,
];

#[rustfmt::skip]
const BISHOP_PST: [Score; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -10,   0,   5,  10,  10,   5,   0, -10,
    -10,   5,   5,  10,  10,   5,   5, -10,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,  10,  10,  10,  10,  10,  10, -10,
    -10,   5,   0,   0,   0,   0,   5, -10,
    -20, -10, -10, -10, -10, -10, -10, -20,
];

#[rustfmt::skip]
const ROOK_PST: [Score; 64] = [
     0,   0,   0,   0,   0,   0,   0,   0,
     5,  10,  10,  10,  10,  10,  10,   5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
     0,   0,   0,   5,   5,   0,   0,   0,
];

#[rustfmt::skip]
const QUEEN_PST: [Score; 64] = [
    -20, -10, -10,  -5,  -5, -10, -10, -20,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -10,   0,   5,   5,   5,   5,   0, -10,
     -5,   0,   5,   5,   5,   5,   0,  -5,
      0,   0,   5,   5,   5,   5,   0,  -5,
    -10,   5,   5,   5,   5,   5,   0, -10,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -20, -10, -10,  -5,  -5, -10, -10, -20,
];

#[rustfmt::skip]
const KING_MIDDLEGAME_PST: [Score; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -10, -20, -20, -20, -20, -20, -20, -10,
     20,  20,   0,   0,   0,   0,  20,  20,
     20,  30,  10,   0,   0,  10,  30,  20,
];

#[rustfmt::skip]
const KING_ENDGAME_PST: [Score; 64] = [
    -50, -40, -30, -20, -20, -30, -40, -50,
    -30, -20, -10,   0,   0, -10, -20, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -30,   0,   0,   0,   0, -30, -30,
    -50, -30, -30, -30, -30, -30, -30, -50,
];

// ============================================================================
// Valores de material (centipawns)
// ============================================================================

const PAWN_VALUE: Score = 100;
const KNIGHT_VALUE: Score = 320;
const BISHOP_VALUE: Score = 330;
const ROOK_VALUE: Score = 500;
const QUEEN_VALUE: Score = 900;

// ============================================================================
// Bônus posicionais extras
// ============================================================================

const BISHOP_PAIR_BONUS: Score = 30;

// ============================================================================
// Avaliador com PST
// ============================================================================

pub struct PstEvaluator;

impl Evaluator for PstEvaluator {
    fn evaluate(&self, board: &Board) -> Score {
        let white = eval_side(board, Color::White);
        let black = eval_side(board, Color::Black);

        let score = white - black;

        // Retornar do ponto de vista do lado a mover
        if board.to_move == Color::White { score } else { -score }
    }

    fn name(&self) -> &str {
        "pst"
    }
}

/// Avalia uma cor: material + PST + bônus.
#[inline]
fn eval_side(board: &Board, color: Color) -> Score {
    let our_pieces = match color {
        Color::White => board.white_pieces,
        Color::Black => board.black_pieces,
    };

    let is_endgame = is_endgame(board);
    let mut score: Score = 0;

    // Peões
    let mut bb = board.pawns & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += PAWN_VALUE + pst_value(&PAWN_PST, sq, color);
        bb &= bb - 1; // clear LSB
    }

    // Cavalos
    bb = board.knights & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += KNIGHT_VALUE + pst_value(&KNIGHT_PST, sq, color);
        bb &= bb - 1;
    }

    // Bispos
    let bishop_bb = board.bishops & our_pieces;
    bb = bishop_bb;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += BISHOP_VALUE + pst_value(&BISHOP_PST, sq, color);
        bb &= bb - 1;
    }
    // Bônus par de bispos
    if bishop_bb.count_ones() >= 2 {
        score += BISHOP_PAIR_BONUS;
    }

    // Torres
    bb = board.rooks & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += ROOK_VALUE + pst_value(&ROOK_PST, sq, color);
        bb &= bb - 1;
    }

    // Damas
    bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += QUEEN_VALUE + pst_value(&QUEEN_PST, sq, color);
        bb &= bb - 1;
    }

    // Rei (usa tabela de middlegame ou endgame)
    bb = board.kings & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        if is_endgame {
            score += pst_value(&KING_ENDGAME_PST, sq, color);
        } else {
            score += pst_value(&KING_MIDDLEGAME_PST, sq, color);
        }
        bb &= bb - 1;
    }

    score
}

/// Consulta PST com espelhamento para Pretas.
/// As tabelas são do ponto de vista Brancas (rank 1 = rows 0..7 da tabela).
/// Para Pretas, espelhamos: sq -> sq ^ 56 (inverte rank).
#[inline(always)]
fn pst_value(table: &[Score; 64], sq: usize, color: Color) -> Score {
    match color {
        Color::White => table[sq],
        Color::Black => table[sq ^ 56],
    }
}

/// Heurística simples de endgame: sem damas, ou material total baixo.
#[inline]
fn is_endgame(board: &Board) -> bool {
    let total_queens = board.queens.count_ones();
    if total_queens == 0 {
        return true;
    }
    // Cada lado com dama mas sem peças menores = endgame
    let white_minor = (board.knights & board.white_pieces).count_ones()
        + (board.bishops & board.white_pieces).count_ones();
    let black_minor = (board.knights & board.black_pieces).count_ones()
        + (board.bishops & board.black_pieces).count_ones();
    let white_has_queen = (board.queens & board.white_pieces).count_ones() > 0;
    let black_has_queen = (board.queens & board.black_pieces).count_ones() > 0;

    // Endgame se ambos têm dama mas sem peças menores extras
    (white_has_queen && white_minor == 0) && (black_has_queen && black_minor == 0)
}
