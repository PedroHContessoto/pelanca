// Ficheiro: src/engine/eval/pst.rs
// Descrição: Avaliação posicional com PST, tapered eval, estrutura de peões e segurança do rei.

use crate::core::board::Board;
use crate::core::types::{Bitboard, Color};
use crate::engine::traits::{Evaluator, Score};
use crate::moves::magic_bitboards::{get_bishop_attacks_magic, get_rook_attacks_magic, get_queen_attacks_magic};
use crate::moves::knight::get_knight_attacks;

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

// Mobility bonuses per square available (MG, EG)
const KNIGHT_MOBILITY: [Score; 2] = [4, 4];    // ~0-8 squares
const BISHOP_MOBILITY: [Score; 2] = [5, 5];    // ~0-13 squares
const ROOK_MOBILITY: [Score; 2] = [2, 4];      // ~0-14 squares
const QUEEN_MOBILITY: [Score; 2] = [1, 2];     // ~0-27 squares

const KNIGHT_OUTPOST_BONUS: Score = 25;
const ROOK_SEVENTH_BONUS: Score = 20;
const BACKWARD_PAWN_PENALTY: Score = -10;

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

        // Mobility (computed once, returns MG/EG separately)
        let (w_mob_mg, w_mob_eg) = eval_mobility(board, Color::White);
        let (b_mob_mg, b_mob_eg) = eval_mobility(board, Color::Black);

        let mg_score = white_mg - black_mg + w_mob_mg - b_mob_mg;
        let eg_score = white_eg - black_eg + w_mob_eg - b_mob_eg;

        // Tapered: interpolate between mg and eg based on phase
        let score = (mg_score * phase + eg_score * (256 - phase)) / 256;

        if board.to_move == Color::White { score } else { -score }
    }

    fn name(&self) -> &str {
        "pst-eval-v3"
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
        // Knight outpost bonus
        if is_knight_outpost(sq as u8, our_pawns, enemy_pawns, color) {
            score += KNIGHT_OUTPOST_BONUS;
        }
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
        let rank = sq / 8;
        score += ROOK_VALUE + pst_value(&ROOK_PST, sq, color);
        if (our_pawns & FILE_MASKS[file]) == 0 {
            if (enemy_pawns & FILE_MASKS[file]) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        // Rook on 7th rank bonus
        let on_seventh = match color {
            Color::White => rank == 6,
            Color::Black => rank == 1,
        };
        if on_seventh { score += ROOK_SEVENTH_BONUS; }
        bb &= bb - 1;
    }

    // Queens
    bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += QUEEN_VALUE + pst_value(&QUEEN_PST, sq, color);
        bb &= bb - 1;
    }

    // King (middlegame PST + safety + attacker penalty)
    bb = board.kings & our_pieces;
    if bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += pst_value(&KING_MIDDLEGAME_PST, sq, color);
        score += eval_king_safety(sq, our_pawns, color);
        score += eval_king_attackers(board, color);
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
        let rank = sq / 8;
        score += ROOK_VALUE + pst_value(&ROOK_PST, sq, color);
        if (our_pawns & FILE_MASKS[file]) == 0 {
            if (enemy_pawns & FILE_MASKS[file]) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        // Rook on 7th rank (even more valuable in endgame)
        let on_seventh = match color {
            Color::White => rank == 6,
            Color::Black => rank == 1,
        };
        if on_seventh { score += ROOK_SEVENTH_BONUS + 10; }
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

                if is_backward_pawn(sq, our_pawns, enemy_pawns, color) {
                    score += BACKWARD_PAWN_PENALTY;
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

/// Avalia segurança do rei: escudo de peões + penalidade por centro + atacantes.
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

    // Open/semi-open file near king penalty
    let king_zone_files: &[usize] = match king_file {
        0 => &[0, 1],
        7 => &[6, 7],
        _ => &[king_file - 1, king_file, king_file + 1],
    };
    for &f in king_zone_files {
        if (our_pawns & FILE_MASKS[f]) == 0 {
            score -= 10; // Open file near king
        }
    }

    score
}

/// Evaluate king danger from enemy attacking pieces (called from mg eval).
/// Returns a penalty (negative) for the defending side.
#[inline]
fn eval_king_attackers(board: &Board, color: Color) -> Score {
    let our_pieces = match color { Color::White => board.white_pieces, Color::Black => board.black_pieces };
    let enemy_pieces = match color { Color::White => board.black_pieces, Color::Black => board.white_pieces };
    let all_pieces = board.white_pieces | board.black_pieces;

    let our_king = board.kings & our_pieces;
    if our_king == 0 { return 0; }
    let king_sq = our_king.trailing_zeros() as u8;
    let king_zone = crate::moves::king::get_king_attacks(king_sq) | (1u64 << king_sq);

    let mut attacker_count = 0i32;
    let mut attack_weight = 0i32;

    // Enemy knights attacking king zone
    let mut bb = board.knights & enemy_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        if (get_knight_attacks(sq) & king_zone) != 0 {
            attacker_count += 1;
            attack_weight += 2;
        }
    }

    // Enemy bishops attacking king zone
    let mut bb = board.bishops & enemy_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        if (get_bishop_attacks_magic(sq, all_pieces) & king_zone) != 0 {
            attacker_count += 1;
            attack_weight += 2;
        }
    }

    // Enemy rooks attacking king zone
    let mut bb = board.rooks & enemy_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        if (get_rook_attacks_magic(sq, all_pieces) & king_zone) != 0 {
            attacker_count += 1;
            attack_weight += 3;
        }
    }

    // Enemy queens attacking king zone
    let mut bb = board.queens & enemy_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        if (get_queen_attacks_magic(sq, all_pieces) & king_zone) != 0 {
            attacker_count += 1;
            attack_weight += 5;
        }
    }

    // Scale penalty quadratically by number of attackers
    if attacker_count >= 2 {
        -(attack_weight * attacker_count * 5)
    } else {
        0
    }
}

/// Compute piece mobility for one side. Returns (mg_bonus, eg_bonus).
#[inline]
fn eval_mobility(board: &Board, color: Color) -> (Score, Score) {
    let our_pieces = match color { Color::White => board.white_pieces, Color::Black => board.black_pieces };
    let all_pieces = board.white_pieces | board.black_pieces;
    // Exclude own pieces + squares attacked by enemy pawns from mobility targets
    let enemy_pawns = board.pawns & match color { Color::White => board.black_pieces, Color::Black => board.white_pieces };
    let pawn_attacks = match color {
        Color::White => ((enemy_pawns & !FILE_MASKS[0]) >> 9) | ((enemy_pawns & !FILE_MASKS[7]) >> 7),
        Color::Black => ((enemy_pawns & !FILE_MASKS[7]) << 9) | ((enemy_pawns & !FILE_MASKS[0]) << 7),
    };
    let safe = !our_pieces & !pawn_attacks;

    let mut mg: Score = 0;
    let mut eg: Score = 0;

    // Knights
    let mut bb = board.knights & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        let moves = (get_knight_attacks(sq) & safe).count_ones() as Score;
        mg += moves * KNIGHT_MOBILITY[0];
        eg += moves * KNIGHT_MOBILITY[1];
    }

    // Bishops
    let mut bb = board.bishops & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        let moves = (get_bishop_attacks_magic(sq, all_pieces) & safe).count_ones() as Score;
        mg += moves * BISHOP_MOBILITY[0];
        eg += moves * BISHOP_MOBILITY[1];
    }

    // Rooks
    let mut bb = board.rooks & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        let moves = (get_rook_attacks_magic(sq, all_pieces) & safe).count_ones() as Score;
        mg += moves * ROOK_MOBILITY[0];
        eg += moves * ROOK_MOBILITY[1];
    }

    // Queens
    let mut bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        let moves = (get_queen_attacks_magic(sq, all_pieces) & safe).count_ones() as Score;
        mg += moves * QUEEN_MOBILITY[0];
        eg += moves * QUEEN_MOBILITY[1];
    }

    (mg, eg)
}

/// Check if a square is an outpost for a knight (supported by own pawn, no enemy pawn can attack it).
#[inline]
fn is_knight_outpost(sq: u8, our_pawns: Bitboard, enemy_pawns: Bitboard, color: Color) -> bool {
    let file = (sq % 8) as usize;
    let rank = sq / 8;

    // Must be on rank 4-6 for white (3-5 for black, 0-indexed)
    let on_outpost_rank = match color {
        Color::White => rank >= 3 && rank <= 5,
        Color::Black => rank >= 2 && rank <= 4,
    };
    if !on_outpost_rank { return false; }

    // No enemy pawn can attack this square (check adjacent files ahead)
    let adj_files = if file > 0 && file < 7 {
        FILE_MASKS[file - 1] | FILE_MASKS[file + 1]
    } else if file == 0 {
        FILE_MASKS[1]
    } else {
        FILE_MASKS[6]
    };

    // Check if any enemy pawn is on adjacent files at same rank or behind (can advance to attack)
    let enemy_can_attack = match color {
        Color::White => {
            let mut mask = 0u64;
            for r in rank..8 { mask |= 0xFFu64 << (r * 8); }
            (enemy_pawns & adj_files & mask) == 0
        }
        Color::Black => {
            let mut mask = 0u64;
            for r in 0..=rank { mask |= 0xFFu64 << (r * 8); }
            (enemy_pawns & adj_files & mask) == 0
        }
    };
    if !enemy_can_attack { return false; }

    // Supported by own pawn
    let support = match color {
        Color::White => {
            let mut sup = 0u64;
            if file > 0 && rank > 0 { sup |= 1u64 << (sq - 9); }
            if file < 7 && rank > 0 { sup |= 1u64 << (sq - 7); }
            sup
        }
        Color::Black => {
            let mut sup = 0u64;
            if file > 0 && rank < 7 { sup |= 1u64 << (sq + 7); }
            if file < 7 && rank < 7 { sup |= 1u64 << (sq + 9); }
            sup
        }
    };

    (our_pawns & support) != 0
}

/// Check if a pawn is backward (no own pawns on adjacent files behind it, and advance is blocked by enemy pawn).
#[inline]
fn is_backward_pawn(sq: u8, our_pawns: Bitboard, enemy_pawns: Bitboard, color: Color) -> bool {
    let file = (sq % 8) as usize;
    let rank = sq / 8;

    let adj_files = ADJACENT_FILES[file];

    // Check if any own pawn is behind or beside on adjacent files
    let behind = match color {
        Color::White => {
            let mut mask = 0u64;
            for r in 0..=rank { mask |= 0xFFu64 << (r * 8); }
            mask
        }
        Color::Black => {
            let mut mask = 0u64;
            for r in rank..8 { mask |= 0xFFu64 << (r * 8); }
            mask
        }
    };

    if (our_pawns & adj_files & behind) != 0 { return false; }

    // The advance square is attacked by enemy pawn
    let advance_sq = match color {
        Color::White => if rank < 7 { sq + 8 } else { return false; },
        Color::Black => if rank > 0 { sq - 8 } else { return false; },
    };

    let adv_file = (advance_sq % 8) as usize;
    let enemy_pawn_attacks = match color {
        Color::White => {
            let mut atk = 0u64;
            if adv_file > 0 { atk |= 1u64 << (advance_sq + 7); }
            if adv_file < 7 { atk |= 1u64 << (advance_sq + 9); }
            atk
        }
        Color::Black => {
            let mut atk = 0u64;
            if adv_file > 0 && advance_sq >= 9 { atk |= 1u64 << (advance_sq - 9); }
            if adv_file < 7 && advance_sq >= 7 { atk |= 1u64 << (advance_sq - 7); }
            atk
        }
    };

    (enemy_pawns & enemy_pawn_attacks) != 0
}

#[inline(always)]
fn pst_value(table: &[Score; 64], sq: usize, color: Color) -> Score {
    match color {
        Color::White => table[sq],
        Color::Black => table[sq ^ 56],
    }
}
