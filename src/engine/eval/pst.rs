// Ficheiro: src/engine/eval/pst.rs
// Descrição: Avaliação posicional com PST, estrutura de peões, segurança do rei e mobilidade.

use crate::core::board::Board;
use crate::core::types::{Bitboard, Color};
use crate::engine::traits::{Evaluator, Score};

// ============================================================================
// Piece-Square Tables (perspectiva Brancas, a1=index 0, h8=index 63)
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
// Material values
// ============================================================================

const PAWN_VALUE: Score = 100;
const KNIGHT_VALUE: Score = 320;
const BISHOP_VALUE: Score = 330;
const ROOK_VALUE: Score = 500;
const QUEEN_VALUE: Score = 900;

// ============================================================================
// Bonus/penalty constants
// ============================================================================

const BISHOP_PAIR_BONUS: Score = 30;
const DOUBLED_PAWN_PENALTY: Score = -15;
const ISOLATED_PAWN_PENALTY: Score = -20;
const PASSED_PAWN_BONUS: [Score; 8] = [0, 10, 20, 40, 60, 100, 150, 0]; // por rank (White perspective: rank 0=1st..7=8th)
const PAWN_SHIELD_BONUS: Score = 12;
const PAWN_SHIELD_MISSING_PENALTY: Score = -15;
const ROOK_OPEN_FILE_BONUS: Score = 20;
const ROOK_SEMI_OPEN_FILE_BONUS: Score = 10;

// ============================================================================
// File masks
// ============================================================================

const FILE_MASKS: [Bitboard; 8] = [
    0x0101010101010101, // file a
    0x0202020202020202, // file b
    0x0404040404040404, // file c
    0x0808080808080808, // file d
    0x1010101010101010, // file e
    0x2020202020202020, // file f
    0x4040404040404040, // file g
    0x8080808080808080, // file h
];

/// Adjacent files mask for each file (used for isolated pawn detection).
const ADJACENT_FILES: [Bitboard; 8] = [
    FILE_MASKS[1],                          // a: only b
    FILE_MASKS[0] | FILE_MASKS[2],          // b: a+c
    FILE_MASKS[1] | FILE_MASKS[3],          // c: b+d
    FILE_MASKS[2] | FILE_MASKS[4],          // d: c+e
    FILE_MASKS[3] | FILE_MASKS[5],          // e: d+f
    FILE_MASKS[4] | FILE_MASKS[6],          // f: e+g
    FILE_MASKS[5] | FILE_MASKS[7],          // g: f+h
    FILE_MASKS[6],                          // h: only g
];

// ============================================================================
// Evaluator
// ============================================================================

pub struct PstEvaluator;

impl Evaluator for PstEvaluator {
    fn evaluate(&self, board: &Board) -> Score {
        let white = eval_side(board, Color::White);
        let black = eval_side(board, Color::Black);

        let score = white - black;

        if board.to_move == Color::White { score } else { -score }
    }

    fn name(&self) -> &str {
        "pst-eval"
    }
}

/// Avalia uma cor: material + PST + estrutura de peões + segurança do rei + mobilidade.
#[inline]
fn eval_side(board: &Board, color: Color) -> Score {
    let our_pieces = match color {
        Color::White => board.white_pieces,
        Color::Black => board.black_pieces,
    };
    let enemy_pieces = match color {
        Color::White => board.black_pieces,
        Color::Black => board.white_pieces,
    };

    let is_endgame = is_endgame(board);
    let mut score: Score = 0;
    let our_pawns = board.pawns & our_pieces;
    let enemy_pawns = board.pawns & enemy_pieces;

    // === Material + PST ===

    // Peões
    let mut bb = our_pawns;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += PAWN_VALUE + pst_value(&PAWN_PST, sq, color);
        bb &= bb - 1;
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
    if bishop_bb.count_ones() >= 2 {
        score += BISHOP_PAIR_BONUS;
    }

    // Torres
    bb = board.rooks & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        let file = sq % 8;
        score += ROOK_VALUE + pst_value(&ROOK_PST, sq, color);
        // Rook on open/semi-open file
        if (our_pawns & FILE_MASKS[file]) == 0 {
            if (enemy_pawns & FILE_MASKS[file]) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }
        bb &= bb - 1;
    }

    // Damas
    bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += QUEEN_VALUE + pst_value(&QUEEN_PST, sq, color);
        bb &= bb - 1;
    }

    // Rei
    bb = board.kings & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        if is_endgame {
            score += pst_value(&KING_ENDGAME_PST, sq, color);
        } else {
            score += pst_value(&KING_MIDDLEGAME_PST, sq, color);
            // King safety: pawn shield (only middlegame)
            score += eval_king_safety(sq, our_pawns, color);
        }
        bb &= bb - 1;
    }

    // === Pawn Structure ===
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

        // Doubled pawns: penalizar peões extras na mesma coluna
        if count > 1 {
            score += DOUBLED_PAWN_PENALTY * (count as Score - 1);
        }

        // Isolated pawns: sem peões amigos em colunas adjacentes
        if count > 0 && (our_pawns & ADJACENT_FILES[file as usize]) == 0 {
            score += ISOLATED_PAWN_PENALTY * count as Score;
        }

        // Passed pawns: sem peões inimigos na coluna ou adjacentes que bloqueiem
        if count > 0 {
            let mut pawns_on_file = our_on_file;
            while pawns_on_file != 0 {
                let sq = pawns_on_file.trailing_zeros() as u8;
                pawns_on_file &= pawns_on_file - 1;

                if is_passed_pawn(sq, enemy_pawns, color) {
                    let rank = match color {
                        Color::White => sq / 8,       // 0=1st rank ... 7=8th
                        Color::Black => 7 - sq / 8,   // espelhado
                    };
                    score += PASSED_PAWN_BONUS[rank as usize];
                }
            }
        }
    }

    score
}

/// Verifica se um peão é passado (sem peões inimigos bloqueando à frente nas colunas adjacentes+própria).
#[inline]
fn is_passed_pawn(sq: u8, enemy_pawns: Bitboard, color: Color) -> bool {
    let file = (sq % 8) as usize;
    let rank = sq / 8;

    // Máscara: colunas (própria + adjacentes) à frente do peão
    let file_mask = FILE_MASKS[file]
        | if file > 0 { FILE_MASKS[file - 1] } else { 0 }
        | if file < 7 { FILE_MASKS[file + 1] } else { 0 };

    let ahead = match color {
        Color::White => {
            if rank >= 7 { return true; }
            let mut mask = 0u64;
            for r in (rank + 1)..8 {
                mask |= 0xFFu64 << (r * 8);
            }
            mask
        }
        Color::Black => {
            if rank == 0 { return true; }
            let mut mask = 0u64;
            for r in 0..rank {
                mask |= 0xFFu64 << (r * 8);
            }
            mask
        }
    };

    (enemy_pawns & file_mask & ahead) == 0
}

/// Avalia segurança do rei: escudo de peões.
#[inline]
fn eval_king_safety(king_sq: usize, our_pawns: Bitboard, color: Color) -> Score {
    let king_file = king_sq % 8;
    let mut score: Score = 0;

    // Avaliar apenas se o rei está nos flancos (castled)
    if king_file > 4 || king_file < 3 {
        // Verificar peões nas colunas do rei e adjacentes
        let shield_files: &[usize] = if king_file <= 2 {
            &[0, 1, 2]
        } else {
            &[5, 6, 7]
        };

        for &f in shield_files {
            let file_pawns = our_pawns & FILE_MASKS[f];
            if file_pawns != 0 {
                // Verificar se há peão na 2ª ou 3ª rank (relativo à cor)
                let shield_ranks = match color {
                    Color::White => FILE_MASKS[f] & (0xFFu64 << 8 | 0xFFu64 << 16), // ranks 2-3
                    Color::Black => FILE_MASKS[f] & (0xFFu64 << 40 | 0xFFu64 << 48), // ranks 6-7
                };
                if (file_pawns & shield_ranks) != 0 {
                    score += PAWN_SHIELD_BONUS;
                } else {
                    score += PAWN_SHIELD_MISSING_PENALTY / 2; // peão avançado demais
                }
            } else {
                score += PAWN_SHIELD_MISSING_PENALTY;
            }
        }
    }

    score
}

/// Consulta PST com espelhamento para Pretas.
#[inline(always)]
fn pst_value(table: &[Score; 64], sq: usize, color: Color) -> Score {
    match color {
        Color::White => table[sq],
        Color::Black => table[sq ^ 56],
    }
}

/// Heurística de endgame.
#[inline]
fn is_endgame(board: &Board) -> bool {
    let total_queens = board.queens.count_ones();
    if total_queens == 0 {
        return true;
    }
    let white_minor = (board.knights & board.white_pieces).count_ones()
        + (board.bishops & board.white_pieces).count_ones();
    let black_minor = (board.knights & board.black_pieces).count_ones()
        + (board.bishops & board.black_pieces).count_ones();
    let white_has_queen = (board.queens & board.white_pieces).count_ones() > 0;
    let black_has_queen = (board.queens & board.black_pieces).count_ones() > 0;

    (white_has_queen && white_minor == 0) && (black_has_queen && black_minor == 0)
}
