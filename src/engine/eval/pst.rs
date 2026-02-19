// Ficheiro: src/engine/eval/pst.rs
// Descrição: Avaliação posicional com PST, tapered eval, estrutura de peões e segurança do rei.

use crate::core::board::Board;
use crate::core::types::{Bitboard, Color};
use crate::engine::traits::{Evaluator, Score};

// ============================================================================
// Piece-Square Tables (perspectiva Brancas, a1=index 0, h8=index 63)
// ============================================================================

#[rustfmt::skip]
const PAWN_PST: [Score; 64] = [
    // rank 1 (a1-h1) — no pawns here
     0,   0,   0,   0,   0,   0,   0,   0,
    // rank 2 (a2-h2) — starting rank for White pawns
     5,  10,  10, -20, -20,  10,  10,   5,
    // rank 3
     5,  -5, -10,   0,   0, -10,  -5,   5,
    // rank 4
     0,   0,   0,  20,  20,   0,   0,   0,
    // rank 5
     5,   5,  10,  25,  25,  10,   5,   5,
    // rank 6
    10,  10,  20,  30,  30,  20,  10,  10,
    // rank 7 — near promotion
    50,  50,  50,  50,  50,  50,  50,  50,
    // rank 8 — no pawns here (promoted)
     0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const KNIGHT_PST: [Score; 64] = [
    // rank 1 (a1-h1)
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,   0,   5,   5,   0, -20, -40,
    -30,   5,  10,  15,  15,  10,   5, -30,
    -30,   0,  15,  20,  20,  15,   0, -30,
    -30,   5,  15,  20,  20,  15,   5, -30,
    -30,   0,  10,  15,  15,  10,   0, -30,
    -40, -20,   0,   0,   0,   0, -20, -40,
    // rank 8 (a8-h8)
    -50, -40, -30, -30, -30, -30, -40, -50,
];

#[rustfmt::skip]
const BISHOP_PST: [Score; 64] = [
    // rank 1 (a1-h1)
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,   5,   0,   0,   0,   0,   5, -10,
    -10,  10,  10,  10,  10,  10,  10, -10,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,   5,   5,  10,  10,   5,   5, -10,
    -10,   0,   5,  10,  10,   5,   0, -10,
    -10,   0,   0,   0,   0,   0,   0, -10,
    // rank 8 (a8-h8)
    -20, -10, -10, -10, -10, -10, -10, -20,
];

#[rustfmt::skip]
const ROOK_PST: [Score; 64] = [
    // rank 1 (a1-h1)
     0,   0,   0,   5,   5,   0,   0,   0,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
     5,  10,  10,  10,  10,  10,  10,   5,
    // rank 8 (a8-h8)
     0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const QUEEN_PST: [Score; 64] = [
    // rank 1 (a1-h1)
    -20, -10, -10,  -5,  -5, -10, -10, -20,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -10,   5,   5,   5,   5,   5,   0, -10,
      0,   0,   5,   5,   5,   5,   0,  -5,
     -5,   0,   5,   5,   5,   5,   0,  -5,
    -10,   0,   5,   5,   5,   5,   0, -10,
    -10,   0,   0,   0,   0,   0,   0, -10,
    // rank 8 (a8-h8)
    -20, -10, -10,  -5,  -5, -10, -10, -20,
];

#[rustfmt::skip]
const KING_MIDDLEGAME_PST: [Score; 64] = [
    // rank 1 (a1-h1) — White king castled, stay on back rank
     20,  30,  10,   0,   0,  10,  30,  20,
     20,  20,   0,   0,   0,   0,  20,  20,
    -10, -20, -20, -20, -20, -20, -20, -10,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    // rank 8 (a8-h8)
    -30, -40, -40, -50, -50, -40, -40, -30,
];

#[rustfmt::skip]
const KING_ENDGAME_PST: [Score; 64] = [
    // rank 1 (a1-h1)
    -50, -30, -30, -30, -30, -30, -30, -50,
    -30, -30,   0,   0,   0,   0, -30, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -20, -10,   0,   0, -10, -20, -30,
    // rank 8 (a8-h8)
    -50, -40, -30, -20, -20, -30, -40, -50,
];

// ============================================================================
// Material values
// ============================================================================

const PAWN_VALUE: Score = 100;
const KNIGHT_VALUE: Score = 320;
const BISHOP_VALUE: Score = 330;
const ROOK_VALUE: Score = 500;
const QUEEN_VALUE: Score = 900;

// ============================================================================
// Tapered eval phase weights
// ============================================================================

const KNIGHT_PHASE: i32 = 1;
const BISHOP_PHASE: i32 = 1;
const ROOK_PHASE: i32 = 2;
const QUEEN_PHASE: i32 = 4;
const TOTAL_PHASE: i32 = 4 * KNIGHT_PHASE + 4 * BISHOP_PHASE + 4 * ROOK_PHASE + 2 * QUEEN_PHASE;

// ============================================================================
// Bonus/penalty constants
// ============================================================================

const BISHOP_PAIR_BONUS: Score = 30;
const DOUBLED_PAWN_PENALTY: Score = -15;
const ISOLATED_PAWN_PENALTY: Score = -20;
const PASSED_PAWN_BONUS: [Score; 8] = [0, 10, 20, 40, 60, 100, 150, 0];
const PAWN_SHIELD_BONUS: Score = 12;
const PAWN_SHIELD_MISSING_PENALTY: Score = -15;
const ROOK_OPEN_FILE_BONUS: Score = 20;
const ROOK_SEMI_OPEN_FILE_BONUS: Score = 10;
const KING_CENTER_PENALTY: Score = -30;

// ============================================================================
// File masks
// ============================================================================

const FILE_MASKS: [Bitboard; 8] = [
    0x0101010101010101, 0x0202020202020202, 0x0404040404040404, 0x0808080808080808,
    0x1010101010101010, 0x2020202020202020, 0x4040404040404040, 0x8080808080808080,
];

const ADJACENT_FILES: [Bitboard; 8] = [
    FILE_MASKS[1],
    FILE_MASKS[0] | FILE_MASKS[2],
    FILE_MASKS[1] | FILE_MASKS[3],
    FILE_MASKS[2] | FILE_MASKS[4],
    FILE_MASKS[3] | FILE_MASKS[5],
    FILE_MASKS[4] | FILE_MASKS[6],
    FILE_MASKS[5] | FILE_MASKS[7],
    FILE_MASKS[6],
];

// ============================================================================
// Evaluator
// ============================================================================

pub struct PstEvaluator;

impl Evaluator for PstEvaluator {
    fn evaluate(&self, board: &Board) -> Score {
        let phase = compute_phase(board);

        let white_mg = eval_side_mg(board, Color::White);
        let white_eg = eval_side_eg(board, Color::White);
        let black_mg = eval_side_mg(board, Color::Black);
        let black_eg = eval_side_eg(board, Color::Black);

        let mg_score = white_mg - black_mg;
        let eg_score = white_eg - black_eg;

        // Tapered: interpolate between mg and eg based on phase
        let score = (mg_score * phase + eg_score * (256 - phase)) / 256;

        if board.to_move == Color::White { score } else { -score }
    }

    fn name(&self) -> &str {
        "pst-eval-v2"
    }
}

/// Compute game phase (256 = full middlegame, 0 = pure endgame).
#[inline]
fn compute_phase(board: &Board) -> Score {
    let mut phase = 0i32;
    phase += board.knights.count_ones() as i32 * KNIGHT_PHASE;
    phase += board.bishops.count_ones() as i32 * BISHOP_PHASE;
    phase += board.rooks.count_ones() as i32 * ROOK_PHASE;
    phase += board.queens.count_ones() as i32 * QUEEN_PHASE;
    // Clamp and scale to 0-256
    let phase = phase.min(TOTAL_PHASE);
    (phase * 256) / TOTAL_PHASE
}

/// Middlegame evaluation for one side.
#[inline]
fn eval_side_mg(board: &Board, color: Color) -> Score {
    let our_pieces = match color { Color::White => board.white_pieces, Color::Black => board.black_pieces };
    let enemy_pieces = match color { Color::White => board.black_pieces, Color::Black => board.white_pieces };
    let our_pawns = board.pawns & our_pieces;
    let enemy_pawns = board.pawns & enemy_pieces;
    let mut score: Score = 0;

    // Pawns
    let mut bb = our_pawns;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += PAWN_VALUE + pst_value(&PAWN_PST, sq, color);
        bb &= bb - 1;
    }

    // Knights
    bb = board.knights & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += KNIGHT_VALUE + pst_value(&KNIGHT_PST, sq, color);
        bb &= bb - 1;
    }

    // Bishops
    let bishop_bb = board.bishops & our_pieces;
    bb = bishop_bb;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += BISHOP_VALUE + pst_value(&BISHOP_PST, sq, color);
        bb &= bb - 1;
    }
    if bishop_bb.count_ones() >= 2 { score += BISHOP_PAIR_BONUS; }

    // Rooks
    bb = board.rooks & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        let file = sq % 8;
        score += ROOK_VALUE + pst_value(&ROOK_PST, sq, color);
        if (our_pawns & FILE_MASKS[file]) == 0 {
            if (enemy_pawns & FILE_MASKS[file]) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        bb &= bb - 1;
    }

    // Queens
    bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += QUEEN_VALUE + pst_value(&QUEEN_PST, sq, color);
        bb &= bb - 1;
    }

    // King (middlegame PST + safety)
    bb = board.kings & our_pieces;
    if bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += pst_value(&KING_MIDDLEGAME_PST, sq, color);
        score += eval_king_safety(sq, our_pawns, color);
    }

    // Pawn structure
    score += eval_pawn_structure(our_pawns, enemy_pawns, color);

    score
}

/// Endgame evaluation for one side.
#[inline]
fn eval_side_eg(board: &Board, color: Color) -> Score {
    let our_pieces = match color { Color::White => board.white_pieces, Color::Black => board.black_pieces };
    let enemy_pieces = match color { Color::White => board.black_pieces, Color::Black => board.white_pieces };
    let our_pawns = board.pawns & our_pieces;
    let enemy_pawns = board.pawns & enemy_pieces;
    let mut score: Score = 0;

    // Pawns
    let mut bb = our_pawns;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += PAWN_VALUE + pst_value(&PAWN_PST, sq, color);
        bb &= bb - 1;
    }

    // Knights
    bb = board.knights & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += KNIGHT_VALUE + pst_value(&KNIGHT_PST, sq, color);
        bb &= bb - 1;
    }

    // Bishops
    let bishop_bb = board.bishops & our_pieces;
    bb = bishop_bb;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += BISHOP_VALUE + pst_value(&BISHOP_PST, sq, color);
        bb &= bb - 1;
    }
    if bishop_bb.count_ones() >= 2 { score += BISHOP_PAIR_BONUS; }

    // Rooks
    bb = board.rooks & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        let file = sq % 8;
        score += ROOK_VALUE + pst_value(&ROOK_PST, sq, color);
        if (our_pawns & FILE_MASKS[file]) == 0 {
            if (enemy_pawns & FILE_MASKS[file]) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        bb &= bb - 1;
    }

    // Queens
    bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += QUEEN_VALUE + pst_value(&QUEEN_PST, sq, color);
        bb &= bb - 1;
    }

    // King (endgame PST, no safety — king should be active)
    bb = board.kings & our_pieces;
    if bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += pst_value(&KING_ENDGAME_PST, sq, color);
    }

    // Pawn structure (passed pawns are more valuable in endgame — bonus already scales with rank)
    score += eval_pawn_structure(our_pawns, enemy_pawns, color);

    score
}

/// Avalia estrutura de peões: dobrados, isolados, passados.
#[inline]
fn eval_pawn_structure(our_pawns: Bitboard, enemy_pawns: Bitboard, color: Color) -> Score {
    let mut score: Score = 0;

    for file in 0..8u8 {
        let our_on_file = our_pawns & FILE_MASKS[file as usize];
        let count = our_on_file.count_ones();

        if count > 1 {
            score += DOUBLED_PAWN_PENALTY * (count as Score - 1);
        }

        if count > 0 && (our_pawns & ADJACENT_FILES[file as usize]) == 0 {
            score += ISOLATED_PAWN_PENALTY * count as Score;
        }

        if count > 0 {
            let mut pawns_on_file = our_on_file;
            while pawns_on_file != 0 {
                let sq = pawns_on_file.trailing_zeros() as u8;
                pawns_on_file &= pawns_on_file - 1;

                if is_passed_pawn(sq, enemy_pawns, color) {
                    let rank = match color {
                        Color::White => sq / 8,
                        Color::Black => 7 - sq / 8,
                    };
                    score += PASSED_PAWN_BONUS[rank as usize];
                }
            }
        }
    }

    score
}

#[inline]
fn is_passed_pawn(sq: u8, enemy_pawns: Bitboard, color: Color) -> bool {
    let file = (sq % 8) as usize;
    let rank = sq / 8;

    let file_mask = FILE_MASKS[file]
        | if file > 0 { FILE_MASKS[file - 1] } else { 0 }
        | if file < 7 { FILE_MASKS[file + 1] } else { 0 };

    let ahead = match color {
        Color::White => {
            if rank >= 7 { return true; }
            let mut mask = 0u64;
            for r in (rank + 1)..8 { mask |= 0xFFu64 << (r * 8); }
            mask
        }
        Color::Black => {
            if rank == 0 { return true; }
            let mut mask = 0u64;
            for r in 0..rank { mask |= 0xFFu64 << (r * 8); }
            mask
        }
    };

    (enemy_pawns & file_mask & ahead) == 0
}

/// Avalia segurança do rei: escudo de peões + penalidade por centro.
#[inline]
fn eval_king_safety(king_sq: usize, our_pawns: Bitboard, color: Color) -> Score {
    let king_file = king_sq % 8;
    let mut score: Score = 0;

    // Penalidade se o rei está no centro (files d-e, ranks 1-2 ou 7-8) durante middlegame
    let king_rank = king_sq / 8;
    let on_back_rank = match color {
        Color::White => king_rank <= 1,
        Color::Black => king_rank >= 6,
    };
    if king_file >= 3 && king_file <= 4 && on_back_rank {
        score += KING_CENTER_PENALTY;
    }

    // Pawn shield only if king is on the flanks (castled)
    if king_file > 4 || king_file < 3 {
        let shield_files: &[usize] = if king_file <= 2 { &[0, 1, 2] } else { &[5, 6, 7] };

        for &f in shield_files {
            let file_pawns = our_pawns & FILE_MASKS[f];
            if file_pawns != 0 {
                let shield_ranks = match color {
                    Color::White => FILE_MASKS[f] & (0xFFu64 << 8 | 0xFFu64 << 16),
                    Color::Black => FILE_MASKS[f] & (0xFFu64 << 40 | 0xFFu64 << 48),
                };
                if (file_pawns & shield_ranks) != 0 {
                    score += PAWN_SHIELD_BONUS;
                } else {
                    score += PAWN_SHIELD_MISSING_PENALTY / 2;
                }
            } else {
                score += PAWN_SHIELD_MISSING_PENALTY;
            }
        }
    }

    score
}

#[inline(always)]
fn pst_value(table: &[Score; 64], sq: usize, color: Color) -> Score {
    match color {
        Color::White => table[sq],
        Color::Black => table[sq ^ 56],
    }
}
