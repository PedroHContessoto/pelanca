// Ficheiro: src/engine/eval/pst.rs
// Descrição: Avaliação posicional completa com tapered eval.
//
// Arquitectura: S(mg, eg)
// ─────────────────────────────────────────────────────────────────────────────
// Cada termo de avaliação produz um par (middlegame, endgame). Os pares são
// acumulados separadamente ao longo de toda a avaliação e interpolados uma
// única vez no final com base na fase de jogo. Isto é a abordagem padrão
// usada por Stockfish, Ethereal, Berserk e praticamente todas as engines
// competitivas modernas.
//
// Componentes de avaliação:
//   1. Material (valores MG/EG distintos)
//   2. Piece-Square Tables (MG e EG separados para cada tipo de peça)
//   3. Estrutura de peões (dobrados, isolados, atrasados, passados, conectados)
//   4. Par de bispos
//   5. Torres em colunas abertas/semi-abertas + torre na 7ª fila
//   6. Cavalos em outposts
//   7. Segurança do rei (escudo de peões, tempestade de peões, colunas abertas)
//   8. Bónus de tempo
//
// Layout de casas: a1 = 0, b1 = 1, ..., h1 = 7, a2 = 8, ..., h8 = 63
// As PSTs estão na perspectiva das Brancas. Para Pretas, espelha-se: sq ^ 56.

use crate::core::board::Board;
use crate::core::types::{Bitboard, Color};
use crate::engine::traits::{Evaluator, Score};

// ============================================================================
// Score pair: (middlegame, endgame) — acumula separadamente, taper no final
// ============================================================================

/// Par de scores (middlegame, endgame) para tapered eval.
#[derive(Clone, Copy, Default)]
struct S(Score, Score);

impl S {
    #[inline(always)]
    const fn new(mg: Score, eg: Score) -> Self {
        S(mg, eg)
    }
    #[inline(always)]
    fn mg(self) -> Score { self.0 }
    #[inline(always)]
    fn eg(self) -> Score { self.1 }
}

impl std::ops::Add for S {
    type Output = S;
    #[inline(always)]
    fn add(self, rhs: S) -> S { S(self.0 + rhs.0, self.1 + rhs.1) }
}

impl std::ops::Sub for S {
    type Output = S;
    #[inline(always)]
    fn sub(self, rhs: S) -> S { S(self.0 - rhs.0, self.1 - rhs.1) }
}

impl std::ops::AddAssign for S {
    #[inline(always)]
    fn add_assign(&mut self, rhs: S) { self.0 += rhs.0; self.1 += rhs.1; }
}

impl std::ops::SubAssign for S {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: S) { self.0 -= rhs.0; self.1 -= rhs.1; }
}

impl std::ops::Mul<Score> for S {
    type Output = S;
    #[inline(always)]
    fn mul(self, rhs: Score) -> S { S(self.0 * rhs, self.1 * rhs) }
}

impl std::ops::Neg for S {
    type Output = S;
    #[inline(always)]
    fn neg(self) -> S { S(-self.0, -self.1) }
}

// ============================================================================
// Material values — MG e EG separados
// ============================================================================
// Fonte: valores calibrados a partir de Stockfish/Ethereal com ajustes.
// Em EG os peões valem mais (promoção), cavalos valem menos (tabuleiro aberto),
// bispos valem mais (diagonais longas), torres valem mais (colunas abertas).

const PAWN_VALUE:   S = S::new(  82, 114);
const KNIGHT_VALUE: S = S::new( 337, 281);
const BISHOP_VALUE: S = S::new( 365, 297);
const ROOK_VALUE:   S = S::new( 477, 512);
const QUEEN_VALUE:  S = S::new(1025, 936);

// ============================================================================
// Piece-Square Tables — MG e EG separados
// ============================================================================
// Perspectiva Brancas, a1=0. Para Pretas espelhar com sq^56.
//
// Princípios por fase:
//   MG: controlo do centro, desenvolvimento, segurança, evitar bordas
//   EG: actividade do rei, peões avançados, centralização geral
//
// Valores calibrados a partir de PeSTO (tuned por Texel em milhões de jogos)
// com ajustes manuais para coerência.

#[rustfmt::skip]
const PAWN_MG: [Score; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,  // rank 1 — sem peões
     -1,  -7, -11, -35, -13,   5,   3,  -5,  // rank 2 — penalizar peões centrais estáticos
     -6,  -8,   5,  11,  11,   5,  -8,  -7,  // rank 3
     -9,  -1,  -8,  21,  23,  -5,  -2, -11,  // rank 4 — bónus d4/e4
    -13,   2,  -5,  12,  17,   6,  10, -13,  // rank 5
     -4,  -2,  14,  24,  26,  16,   2,  -6,  // rank 6
     98, 134,  61,  95,  68, 126, 134, 100,  // rank 7 — quase promoção (valores altos)
      0,   0,   0,   0,   0,   0,   0,   0,  // rank 8 — promovido
];

#[rustfmt::skip]
const PAWN_EG: [Score; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
     15,  15,  17,  21,  21,  17,  15,  15,  // EG: peões centrais valem mais
     12,  16,   6,   3,   3,   6,  16,  12,
     10,  17,  14,  13,  13,  14,  17,  10,
     22,  25,  22,  20,  20,  22,  25,  22,
     56,  63,  55,  48,  48,  55,  63,  56,
    178, 173, 158, 134, 134, 158, 173, 178,  // EG: promoção iminente — muito valiosos
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const KNIGHT_MG: [Score; 64] = [
    -105, -21, -58, -33, -17, -28, -19, -23,  // rank 1 — cavalos na borda/canto é péssimo
     -29, -53, -12,  -3,  -1,  18, -14, -19,
     -23,  -9,  12,  10,  19,  17,  25, -16,
     -13,   4,  16,  13,  28,  19,  21,  -8,
      -9,  17,  19,  53,  37,  69,  18,  22,
     -47,  60,  37,  65,  84, 129,  73,  44,
     -73, -41,  72,  36,  23,  62,   7, -17,
    -167, -89, -34, -49,  61, -97,  50, -75,  // rank 8 — raramente bom
];

#[rustfmt::skip]
const KNIGHT_EG: [Score; 64] = [
     -29, -51, -23, -15, -22, -18, -50, -64,  // EG: centralização importa
     -42, -20, -10,  -5,  -2, -20, -23, -44,
     -23,  -3,  -1,  15,  10,  -3, -20, -22,
     -18,  -6,  16,  25,  16,  17,   4, -18,
     -17,   3,  22,  22,  22,  11,   8, -18,
     -24, -20,  10,   9,  -1,  -9, -19, -41,
     -25,  -8, -25,  -2,  -9, -25, -24, -52,
     -58, -38, -13, -28, -31, -27, -63, -99,
];

#[rustfmt::skip]
const BISHOP_MG: [Score; 64] = [
     -33,  -3, -14, -21, -13, -12, -39, -21,  // rank 1
       4,  15,  16,   0,   7,  21,  33,   1,
       0,  15,  15,  15,  14,  27,  18,  10,
      -6,  13,  13,  26,  34,  12,  10,   4,
      -4,   5,  19,  50,  37,  37,   7,  -2,
     -16,  37,  43,  40,  35,  50,  37,  -2,
     -26,  16, -18, -13,  30,  59,  18, -47,
     -29,   4, -82, -37, -25, -42,   7,  -8,  // rank 8
];

#[rustfmt::skip]
const BISHOP_EG: [Score; 64] = [
     -23,  -9, -23,  -5,  -9, -16,  -5, -17,
     -14, -18,  -7,  -1,   4,  -9, -15, -27,
     -12,  -3,   8,  10,  13,   3,  -7, -15,
      -6,   3,  13,  19,   7,  10,  -3,  -9,
      -3,   9,  12,   9,  14,  10,   3,   2,
       2,  -8,   0,  -1,  -2,   6,   0,   4,
      -8,  -4,   7, -12,  -3, -13,  -4, -14,
     -14, -21, -11,  -8, -14,  -5, -14, -16,
];

#[rustfmt::skip]
const ROOK_MG: [Score; 64] = [
     -19, -13,   1,  17,  16,   7, -37, -26,  // rank 1 — centralizar, apoiar roque
     -44, -16, -20,  -9,  -1,  11,  -6, -71,
     -45, -25, -16, -17,   3,   0, -16, -28,
     -36, -26, -12,  -1,   9,  -7,   6, -23,
     -24, -11,   7,  26,  24,  35,  -8, -20,
      -5,  19,  26,  36,  17,  45,  61,  16,
      27,  32,  58,  62,  80,  67,  26,  44,
      32,  42,  32,  51,  63,   9,  31,  43,  // rank 8
];

#[rustfmt::skip]
const ROOK_EG: [Score; 64] = [
     -9,   2,   3,  -1,  -5,  -13,  4, -20,  // EG: actividade geral
     -6,  -6,   0,   2,  -9,  -9, -11,  -3,
     -4,   0,  -5,  -1,  -7, -12,  -8, -16,
      3,   5,   8,   4,  -5,  -6,  -8, -11,
      4,   3,  13,   1,   2,   1,  -1,   2,
      0,   0,   0,   7,   0,  -2,  -1,  -2,
     -2,  -5,   8,  10,   5,   5, -10,  -6,
      8,  11,  13,  18,  -3,   3,  -4,  -1,
];

#[rustfmt::skip]
const QUEEN_MG: [Score; 64] = [
     -1, -18,  -9,  10, -15, -25, -31, -50,  // rank 1 — dama não deve sair cedo
     -35,  -8,  11,   2,   8,  15,  -3,   1,
     -14,   2, -11,  -2,  -5,   2,  14,   5,
      -9, -26,  -9, -10,  -2,  -4,   3,  -3,
     -27, -27, -16, -16,  -1,  17,  -2,   1,
     -13, -17,   7,   8,  29,  56,  47,  57,
     -24, -39,  -5,   1, -16,  57,  28,  54,
     -28,   0,  29,  12,  59,  44,  43,  45,  // rank 8
];

#[rustfmt::skip]
const QUEEN_EG: [Score; 64] = [
     -33, -28, -22, -43,  -5, -32, -20, -41,
     -22, -23, -30, -16, -16, -23, -36, -32,
     -16, -27,  15,   6,   9,  17,  10,   5,
     -18,  28,  19,  47,  31,  34,  39,  23,
       3,  22,  24,  45,  57,  40,  57,  36,
     -20,   6,   9,  49,  47,  35,  19,   9,
     -17,  20,  32,  41,  58,  25,  30,   0,
      -9,  22,  22,  27,  27,  19,  10,  20,
];

#[rustfmt::skip]
const KING_MG: [Score; 64] = [
     -15,  36,  12, -54,   8, -28,  24,  14,  // rank 1 — rei quer estar roqueado
       1,   7,  -8, -64, -43, -16,   9,   8,
     -14, -14, -22, -46, -44, -30, -15, -27,
     -49,  -1, -27, -39, -46, -44, -33, -51,
     -17, -20, -12, -27, -30, -25, -14, -36,
      -9,  24,   2, -16, -20,   6,  22, -22,
      29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -65,  23,  16, -15, -56, -34,   2,  13,  // rank 8
];

#[rustfmt::skip]
const KING_EG: [Score; 64] = [
     -53, -34, -21, -11, -28, -14, -24, -43,  // EG: rei deve centralizar-se
     -27, -11,   4,  13,  14,   4,  -5, -17,
     -19,  -3,  11,  21,  23,  16,   7,  -9,
     -18,  -4,  21,  24,  27,  23,   9, -11,
      -8,  22,  24,  27,  26,  33,  26,   3,
      10,  17,  23,  15,  20,  23,  20, -16,
     -18,   7,  14,  12,  13,  14,  -1, -14,
     -35,  -1,  -7,  -2,   6, -10,   3, -28,
];

// ============================================================================
// Phase computation weights
// ============================================================================

const KNIGHT_PHASE: i32 = 1;
const BISHOP_PHASE: i32 = 1;
const ROOK_PHASE: i32 = 2;
const QUEEN_PHASE: i32 = 4;
const TOTAL_PHASE: i32 = 4 * KNIGHT_PHASE + 4 * BISHOP_PHASE + 4 * ROOK_PHASE + 2 * QUEEN_PHASE;

// ============================================================================
// Evaluation bonus/penalty constants — todos como S(mg, eg)
// ============================================================================

// --- Par de bispos ---
// O par de bispos vale mais no endgame (tabuleiro aberto, diagonais longas).
const BISHOP_PAIR_BONUS: S = S::new(23, 62);

// --- Estrutura de peões ---
const DOUBLED_PAWN_PENALTY: S  = S::new(-11, -56);  // EG muito pior (lentos, bloqueiam-se)
const ISOLATED_PAWN_PENALTY: S = S::new( -5, -15);  // Fraqueza posicional persistente
const BACKWARD_PAWN_PENALTY: S = S::new( -9, -24);  // Não pode avançar com segurança

// Peões passados por rank relativo (0=rank1, 7=rank8 — irrelevante para 0 e 7)
// MG: bónus moderado; EG: bónus exponencial (promoção!)
const PASSED_PAWN_BONUS: [S; 8] = [
    S::new(  0,   0), // rank 1 — impossível
    S::new(  2,  10), // rank 2 — acabou de avançar
    S::new(  5,  17), // rank 3
    S::new( 12,  33), // rank 4
    S::new( 32,  72), // rank 5 — perigoso
    S::new( 70, 177), // rank 6 — muito perigoso
    S::new(172, 260), // rank 7 — quase promove
    S::new(  0,   0), // rank 8 — promovido
];

// Bónus se peões passados estão conectados (protegem-se mutuamente)
const CONNECTED_PASSER_BONUS: S = S::new(7, 21);

// --- Torres ---
const ROOK_OPEN_FILE_BONUS: S      = S::new(47, 21);  // Coluna sem peões nossos nem adversários
const ROOK_SEMI_OPEN_FILE_BONUS: S = S::new(20, 10);  // Coluna sem peões nossos
const ROOK_ON_SEVENTH_BONUS: S     = S::new(11, 26);  // Torre na 7ª fila (peões adversários presos)

// --- Cavalos ---
// Outpost: cavalo protegido por peão numa casa que não pode ser atacada por peões adversários
const KNIGHT_OUTPOST_BONUS: S = S::new(30, 18);
// Bónus extra se o outpost está no centro
const KNIGHT_OUTPOST_CENTER_BONUS: S = S::new(16, 6);

// --- Segurança do rei (apenas MG — no EG o rei deve estar activo) ---
const PAWN_SHIELD_BONUS: S  = S::new(17, 0);    // Peão na posição de escudo
const PAWN_SHIELD_MISSING: S = S::new(-23, 0);   // Falta um peão no escudo
const KING_OPEN_FILE_PENALTY: S   = S::new(-41, -9);   // Coluna aberta adjacente ao rei
const KING_SEMI_OPEN_FILE_PENALTY: S = S::new(-18, -3); // Coluna semi-aberta adjacente ao rei
const PAWN_STORM_BONUS: S = S::new(7, 0);       // Peão adversário avançando contra o rei

// --- Tempo ---
// Pequeno bónus para o lado a mover (tem a iniciativa).
const TEMPO_BONUS: S = S::new(28, 15);

// ============================================================================
// Bitboard masks — pré-computados
// ============================================================================

const FILE_MASKS: [Bitboard; 8] = [
    0x0101_0101_0101_0101, 0x0202_0202_0202_0202,
    0x0404_0404_0404_0404, 0x0808_0808_0808_0808,
    0x1010_1010_1010_1010, 0x2020_2020_2020_2020,
    0x4040_4040_4040_4040, 0x8080_8080_8080_8080,
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

/// Rank masks: RANK_MASKS[0] = rank 1 (0x00..FF), etc.
const RANK_MASKS: [Bitboard; 8] = [
    0x0000_0000_0000_00FF,
    0x0000_0000_0000_FF00,
    0x0000_0000_00FF_0000,
    0x0000_0000_FF00_0000,
    0x0000_00FF_0000_0000,
    0x0000_FF00_0000_0000,
    0x00FF_0000_0000_0000,
    0xFF00_0000_0000_0000,
];

// ============================================================================
// Passed pawn masks — pré-computados com bit shifts (sem loops em runtime)
// ============================================================================

/// Para Brancas: WHITE_PASSED_MASKS[sq] = todas as casas à frente nas 3 colunas.
/// Se nenhum peão preto está nesta máscara, o peão em sq é passado.
const fn compute_white_passed_masks() -> [Bitboard; 64] {
    let mut masks = [0u64; 64];
    let mut sq = 0u8;
    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        if rank < 7 {
            // Combinar as colunas relevantes
            let mut file_mask = FILE_MASKS[file as usize];
            if file > 0 { file_mask |= FILE_MASKS[(file - 1) as usize]; }
            if file < 7 { file_mask |= FILE_MASKS[(file + 1) as usize]; }
            // Apenas ranks acima: limpar ranks 0..=rank
            // Máscara de ranks acima = !((1 << ((rank+1)*8)) - 1)
            let below_mask = if rank < 7 { (1u64 << ((rank as u32 + 1) * 8)) - 1 } else { u64::MAX };
            masks[sq as usize] = file_mask & !below_mask;
        }
        sq += 1;
    }
    masks
}

const fn compute_black_passed_masks() -> [Bitboard; 64] {
    let mut masks = [0u64; 64];
    let mut sq = 0u8;
    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        if rank > 0 {
            let mut file_mask = FILE_MASKS[file as usize];
            if file > 0 { file_mask |= FILE_MASKS[(file - 1) as usize]; }
            if file < 7 { file_mask |= FILE_MASKS[(file + 1) as usize]; }
            // Apenas ranks abaixo: limpar ranks rank..=7
            let below_mask = (1u64 << (rank as u32 * 8)) - 1;
            masks[sq as usize] = file_mask & below_mask;
        }
        sq += 1;
    }
    masks
}

const WHITE_PASSED_MASKS: [Bitboard; 64] = compute_white_passed_masks();
const BLACK_PASSED_MASKS: [Bitboard; 64] = compute_black_passed_masks();

/// Outpost masks: casas que não podem ser atacadas por peões adversários.
/// Para Brancas: se um cavalo está em sq e não há peões pretos nas colunas adjacentes
/// atrás ou ao nível desta casa, é um outpost.
/// WHITE_OUTPOST_MASK[sq] = colunas adjacentes, ranks >= rank(sq) para Pretas
/// (se vazio de peões pretos, a casa é um outpost).
const fn compute_white_outpost_masks() -> [Bitboard; 64] {
    let mut masks = [0u64; 64];
    let mut sq = 0u8;
    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        // Colunas adjacentes apenas
        let mut adj_files = 0u64;
        if file > 0 { adj_files |= FILE_MASKS[(file - 1) as usize]; }
        if file < 7 { adj_files |= FILE_MASKS[(file + 1) as usize]; }
        // Para ser outpost branco: nenhum peão preto nas colunas adjacentes que possa
        // atacar esta casa. Isso significa ranks >= rank(sq) (peões pretos avançam para baixo).
        // Na verdade, peões pretos atacam de rank+1, então precisamos de ranks rank..=7
        if rank > 0 {
            let ahead_mask = !((1u64 << (rank as u32 * 8)) - 1); // rank e acima
            masks[sq as usize] = adj_files & ahead_mask;
        }
        sq += 1;
    }
    masks
}

const fn compute_black_outpost_masks() -> [Bitboard; 64] {
    let mut masks = [0u64; 64];
    let mut sq = 0u8;
    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        let mut adj_files = 0u64;
        if file > 0 { adj_files |= FILE_MASKS[(file - 1) as usize]; }
        if file < 7 { adj_files |= FILE_MASKS[(file + 1) as usize]; }
        if rank < 7 {
            let below_mask = (1u64 << ((rank as u32 + 1) * 8)) - 1; // ranks abaixo
            masks[sq as usize] = adj_files & below_mask;
        }
        sq += 1;
    }
    masks
}

const WHITE_OUTPOST_MASKS: [Bitboard; 64] = compute_white_outpost_masks();
const BLACK_OUTPOST_MASKS: [Bitboard; 64] = compute_black_outpost_masks();

/// Pawn attack masks: casas atacadas por um peão em sq.
/// (Úteis para futura avaliação de mobilidade e controlo de espaço.)
#[allow(dead_code)]
const fn compute_white_pawn_attacks() -> [Bitboard; 64] {
    let mut attacks = [0u64; 64];
    let mut sq = 0u8;
    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        if rank < 7 {
            if file > 0 { attacks[sq as usize] |= 1u64 << (sq + 7); }
            if file < 7 { attacks[sq as usize] |= 1u64 << (sq + 9); }
        }
        sq += 1;
    }
    attacks
}

#[allow(dead_code)]
const fn compute_black_pawn_attacks() -> [Bitboard; 64] {
    let mut attacks = [0u64; 64];
    let mut sq = 0u8;
    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        if rank > 0 {
            if file > 0 { attacks[sq as usize] |= 1u64 << (sq - 9); }
            if file < 7 { attacks[sq as usize] |= 1u64 << (sq - 7); }
        }
        sq += 1;
    }
    attacks
}

#[allow(dead_code)]
const WHITE_PAWN_ATTACKS: [Bitboard; 64] = compute_white_pawn_attacks();
#[allow(dead_code)]
const BLACK_PAWN_ATTACKS: [Bitboard; 64] = compute_black_pawn_attacks();

// ============================================================================
// Helper: casas centrais para bónus de outpost
// ============================================================================

/// Casas c3-f3, c4-f4, c5-f5, c6-f6 — centro expandido
const CENTER_SQUARES: Bitboard =
    0x00_00_3C_3C_3C_3C_00_00;

// ============================================================================
// Evaluator
// ============================================================================

pub struct PstEvaluator;

impl Evaluator for PstEvaluator {
    fn evaluate(&self, board: &Board) -> Score {
        let phase = compute_phase(board);

        let white_score = eval_side(board, Color::White);
        let black_score = eval_side(board, Color::Black);

        let total = white_score - black_score + TEMPO_BONUS;

        // Tapered eval: interpolação MG/EG
        let score = (total.mg() * phase + total.eg() * (256 - phase)) / 256;

        if board.to_move == Color::White { score } else { -score }
    }

    fn name(&self) -> &str {
        "pst-eval-v3"
    }
}

// ============================================================================
// Phase computation
// ============================================================================

/// Calcula fase do jogo: 256 = middlegame puro, 0 = endgame puro.
#[inline]
fn compute_phase(board: &Board) -> Score {
    let mut phase = 0i32;
    phase += board.knights.count_ones() as i32 * KNIGHT_PHASE;
    phase += board.bishops.count_ones() as i32 * BISHOP_PHASE;
    phase += board.rooks.count_ones() as i32 * ROOK_PHASE;
    phase += board.queens.count_ones() as i32 * QUEEN_PHASE;
    let phase = phase.min(TOTAL_PHASE);
    (phase * 256) / TOTAL_PHASE
}

// ============================================================================
// Evaluation per side — acumula S(mg, eg) para cada componente
// ============================================================================

/// Avaliação completa para um lado. Retorna S(mg, eg).
#[inline]
fn eval_side(board: &Board, color: Color) -> S {
    let our_pieces = match color {
        Color::White => board.white_pieces,
        Color::Black => board.black_pieces,
    };
    let enemy_pieces = match color {
        Color::White => board.black_pieces,
        Color::Black => board.white_pieces,
    };
    let our_pawns = board.pawns & our_pieces;
    let enemy_pawns = board.pawns & enemy_pieces;
    let all_pawns = board.pawns;

    let mut score = S::default();

    // ── 1. Material + PST ──────────────────────────────────────────────────

    // Peões
    let mut bb = our_pawns;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += PAWN_VALUE + pst_pair(&PAWN_MG, &PAWN_EG, sq, color);
        bb &= bb - 1;
    }

    // Cavalos
    bb = board.knights & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += KNIGHT_VALUE + pst_pair(&KNIGHT_MG, &KNIGHT_EG, sq, color);

        // Outpost evaluation
        score += eval_knight_outpost(sq, our_pawns, enemy_pawns, color);

        bb &= bb - 1;
    }

    // Bispos
    let bishop_bb = board.bishops & our_pieces;
    bb = bishop_bb;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += BISHOP_VALUE + pst_pair(&BISHOP_MG, &BISHOP_EG, sq, color);
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
        let rank = sq / 8;
        score += ROOK_VALUE + pst_pair(&ROOK_MG, &ROOK_EG, sq, color);

        // Coluna aberta / semi-aberta
        if (our_pawns & FILE_MASKS[file]) == 0 {
            if (enemy_pawns & FILE_MASKS[file]) == 0 {
                score += ROOK_OPEN_FILE_BONUS;
            } else {
                score += ROOK_SEMI_OPEN_FILE_BONUS;
            }
        }

        // Torre na 7ª fila (relativa)
        let on_seventh = match color {
            Color::White => rank == 6,
            Color::Black => rank == 1,
        };
        if on_seventh {
            score += ROOK_ON_SEVENTH_BONUS;
        }

        bb &= bb - 1;
    }

    // Damas
    bb = board.queens & our_pieces;
    while bb != 0 {
        let sq = bb.trailing_zeros() as usize;
        score += QUEEN_VALUE + pst_pair(&QUEEN_MG, &QUEEN_EG, sq, color);
        bb &= bb - 1;
    }

    // Rei (PST MG + EG já separados, segurança do rei contribui quase só para MG)
    bb = board.kings & our_pieces;
    if bb != 0 {
        let king_sq = bb.trailing_zeros() as usize;
        score += pst_pair(&KING_MG, &KING_EG, king_sq, color);
        score += eval_king_safety(king_sq, our_pawns, enemy_pawns, all_pawns, color);
    }

    // ── 2. Estrutura de peões ──────────────────────────────────────────────
    score += eval_pawn_structure(our_pawns, enemy_pawns, color);

    score
}

// ============================================================================
// Pawn structure evaluation
// ============================================================================

/// Avalia estrutura de peões: dobrados, isolados, atrasados, passados, conectados.
#[inline]
fn eval_pawn_structure(our_pawns: Bitboard, enemy_pawns: Bitboard, color: Color) -> S {
    let mut score = S::default();

    for file in 0..8usize {
        let our_on_file = our_pawns & FILE_MASKS[file];
        let count = our_on_file.count_ones();

        // Peões dobrados: penalidade por cada peão extra na mesma coluna
        if count > 1 {
            score += DOUBLED_PAWN_PENALTY * (count as Score - 1);
        }

        // Peões isolados: sem peões nas colunas adjacentes
        if count > 0 && (our_pawns & ADJACENT_FILES[file]) == 0 {
            score += ISOLATED_PAWN_PENALTY * count as Score;
        }
    }

    // Análise por peão individual: passados, atrasados, conectados
    let mut bb = our_pawns;
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;

        let file = (sq % 8) as usize;
        let rank = sq / 8;
        let rel_rank = match color {
            Color::White => rank,
            Color::Black => 7 - rank,
        };

        // ── Peão passado ──
        let is_passed = is_passed_pawn(sq as usize, enemy_pawns, color);
        if is_passed {
            score += PASSED_PAWN_BONUS[rel_rank as usize];

            // Conectado: outro peão nosso nas colunas adjacentes no mesmo rank ou rank-1
            let connected = (our_pawns & ADJACENT_FILES[file] & (RANK_MASKS[rank as usize]
                | if rel_rank > 0 {
                    match color {
                        Color::White => if rank > 0 { RANK_MASKS[(rank - 1) as usize] } else { 0 },
                        Color::Black => if rank < 7 { RANK_MASKS[(rank + 1) as usize] } else { 0 },
                    }
                  } else { 0 }
            )) != 0;
            if connected {
                score += CONNECTED_PASSER_BONUS;
            }
        }

        // ── Peão atrasado ──
        // Um peão é atrasado se: não é isolado, não é passado, e nenhum peão nosso
        // nas colunas adjacentes está no mesmo rank ou atrás, E a casa à frente
        // é controlada por um peão adversário.
        if !is_passed && (our_pawns & ADJACENT_FILES[file]) != 0 {
            // Verificar se não há peões amigos atrás ou ao lado
            let adjacent_friendly = our_pawns & ADJACENT_FILES[file];
            let friend_behind = match color {
                Color::White => {
                    let below = if rank > 0 { (1u64 << (rank * 8)) - 1 } else { 0 };
                    let same_and_below = below | RANK_MASKS[rank as usize];
                    adjacent_friendly & same_and_below
                }
                Color::Black => {
                    let above = !((1u64 << ((rank + 1) * 8)) - 1);
                    let same_and_above = above | RANK_MASKS[rank as usize];
                    adjacent_friendly & same_and_above
                }
            };

            if friend_behind == 0 {
                // Casa à frente atacada por peão adversário?
                let advance_sq = match color {
                    Color::White => if rank < 7 { sq + 8 } else { sq },
                    Color::Black => if rank > 0 { sq - 8 } else { sq },
                };
                let enemy_controls = match color {
                    Color::White => {
                        // Peões pretos atacam de cima para baixo
                        let asq = advance_sq as usize;
                        let af = asq % 8;
                        let mut ctrl = 0u64;
                        if af > 0 && asq + 7 < 64 { ctrl |= 1u64 << (asq + 7); }
                        if af < 7 && asq + 9 < 64 { ctrl |= 1u64 << (asq + 9); }
                        (enemy_pawns & ctrl) != 0
                    }
                    Color::Black => {
                        let asq = advance_sq as usize;
                        let af = asq % 8;
                        let mut ctrl = 0u64;
                        if af > 0 && asq >= 9 { ctrl |= 1u64 << (asq - 9); }
                        if af < 7 && asq >= 7 { ctrl |= 1u64 << (asq - 7); }
                        (enemy_pawns & ctrl) != 0
                    }
                };

                if enemy_controls {
                    score += BACKWARD_PAWN_PENALTY;
                }
            }
        }
    }

    score
}

/// Verifica se um peão é passado usando máscaras pré-computadas.
#[inline(always)]
fn is_passed_pawn(sq: usize, enemy_pawns: Bitboard, color: Color) -> bool {
    let mask = match color {
        Color::White => WHITE_PASSED_MASKS[sq],
        Color::Black => BLACK_PASSED_MASKS[sq],
    };
    (enemy_pawns & mask) == 0
}

// ============================================================================
// Knight outpost evaluation
// ============================================================================

/// Avalia se um cavalo está num outpost.
/// Outpost = casa protegida por peão nosso, que não pode ser atacada por peões adversários.
#[inline]
fn eval_knight_outpost(sq: usize, our_pawns: Bitboard, enemy_pawns: Bitboard, color: Color) -> S {
    let rank = sq / 8;
    let rel_rank = match color {
        Color::White => rank,
        Color::Black => 7 - rank,
    };

    // Outposts são relevantes nos ranks 4-6 (relativos)
    if rel_rank < 3 || rel_rank > 5 {
        return S::default();
    }

    // Verificar se está protegido por peão nosso
    let protected_by_pawn = match color {
        Color::White => {
            // Peão branco ataca de baixo: um peão em sq-7 ou sq-9 protege sq
            let mut protected = false;
            let file = sq % 8;
            if file > 0 && sq >= 9 { protected |= (our_pawns & (1u64 << (sq - 9))) != 0; }
            if file < 7 && sq >= 7 { protected |= (our_pawns & (1u64 << (sq - 7))) != 0; }
            protected
        }
        Color::Black => {
            let mut protected = false;
            let file = sq % 8;
            if file > 0 && sq + 7 < 64 { protected |= (our_pawns & (1u64 << (sq + 7))) != 0; }
            if file < 7 && sq + 9 < 64 { protected |= (our_pawns & (1u64 << (sq + 9))) != 0; }
            protected
        }
    };

    if !protected_by_pawn {
        return S::default();
    }

    // Verificar se peões adversários não podem atacar esta casa
    let outpost_mask = match color {
        Color::White => WHITE_OUTPOST_MASKS[sq],
        Color::Black => BLACK_OUTPOST_MASKS[sq],
    };

    if (enemy_pawns & outpost_mask) != 0 {
        return S::default(); // Peão adversário pode expulsar o cavalo
    }

    let mut bonus = KNIGHT_OUTPOST_BONUS;
    if (1u64 << sq) & CENTER_SQUARES != 0 {
        bonus += KNIGHT_OUTPOST_CENTER_BONUS;
    }

    bonus
}

// ============================================================================
// King safety — contribui primariamente para MG
// ============================================================================

/// Avalia segurança do rei: escudo de peões, colunas abertas, tempestade adversária.
#[inline]
fn eval_king_safety(
    king_sq: usize,
    our_pawns: Bitboard,
    enemy_pawns: Bitboard,
    _all_pawns: Bitboard,
    color: Color,
) -> S {
    let king_file = king_sq % 8;
    let king_rank = king_sq / 8;
    let mut score = S::default();

    // Determinar se o rei está roqueado (flancos) ou no centro
    let on_kingside = king_file >= 5;
    let on_queenside = king_file <= 2;
    let on_back = match color {
        Color::White => king_rank <= 1,
        Color::Black => king_rank >= 6,
    };

    // Segurança do rei apenas faz sentido se o rei está nos flancos (roqueado)
    if (on_kingside || on_queenside) && on_back {
        // ── Escudo de peões ──
        let shield_files = if on_queenside {
            [king_file, king_file + 1, if king_file + 2 <= 7 { king_file + 2 } else { king_file + 1 }]
        } else {
            [if king_file >= 2 { king_file - 2 } else { 0 }, if king_file >= 1 { king_file - 1 } else { 0 }, king_file]
        };

        for &f in &shield_files {
            let file_pawns = our_pawns & FILE_MASKS[f];
            if file_pawns != 0 {
                // Verificar se há peão nas ranks de escudo (2ª e 3ª do nosso lado)
                let shield_zone = match color {
                    Color::White => RANK_MASKS[1] | RANK_MASKS[2],
                    Color::Black => RANK_MASKS[5] | RANK_MASKS[6],
                };
                if (file_pawns & shield_zone) != 0 {
                    score += PAWN_SHIELD_BONUS;
                } else {
                    // Peão avançou — escudo enfraquecido
                    score += S::new(PAWN_SHIELD_MISSING.mg() / 2, 0);
                }
            } else {
                score += PAWN_SHIELD_MISSING;
            }
        }

        // ── Colunas abertas/semi-abertas adjacentes ao rei ──
        let king_zone_files = if king_file == 0 {
            [0usize, 1]
        } else if king_file == 7 {
            [6, 7]
        } else {
            [king_file - 1, king_file + 1]
        };

        for &f in &king_zone_files {
            if f > 7 { continue; }
            let our_on_file = our_pawns & FILE_MASKS[f];
            let enemy_on_file = enemy_pawns & FILE_MASKS[f];
            if our_on_file == 0 {
                if enemy_on_file == 0 {
                    score += KING_OPEN_FILE_PENALTY;
                } else {
                    score += KING_SEMI_OPEN_FILE_PENALTY;
                }
            }
        }

        // ── Tempestade de peões adversários ──
        // Penalidade se peões adversários estão avançando contra o nosso rei.
        // Quanto mais avançado o peão inimigo, maior a penalidade.
        let storm_files = if on_queenside { [0, 1, 2] } else { [5, 6, 7] };
        for &f in &storm_files {
            let enemy_on_file = enemy_pawns & FILE_MASKS[f];
            if enemy_on_file != 0 {
                // Encontrar o peão mais avançado do adversário (mais perto do nosso rei)
                let most_advanced_sq = match color {
                    // Contra rei branco (ranks 0-1): peão preto mais avançado = menor rank
                    Color::White => enemy_on_file.trailing_zeros() as usize,
                    // Contra rei preto (ranks 6-7): peão branco mais avançado = maior rank
                    Color::Black => 63 - enemy_on_file.leading_zeros() as usize,
                };
                let advance_rank = most_advanced_sq / 8;
                // Distância avançada do peão inimigo (quanto avançou da posição inicial)
                let dist = match color {
                    Color::White => 6usize.saturating_sub(advance_rank), // preto começou rank 6
                    Color::Black => advance_rank.saturating_sub(1),      // branco começou rank 1
                };
                // dist >= 2 = peão avançou significativamente, penalizar
                if dist >= 2 {
                    score -= PAWN_STORM_BONUS * (dist as Score);
                }
            }
        }
    }

    score
}

// ============================================================================
// PST helper
// ============================================================================

/// Retorna S(mg, eg) para uma casa, espelhando para Pretas.
#[inline(always)]
fn pst_pair(mg_table: &[Score; 64], eg_table: &[Score; 64], sq: usize, color: Color) -> S {
    let idx = match color {
        Color::White => sq,
        Color::Black => sq ^ 56,
    };
    S::new(mg_table[idx], eg_table[idx])
}