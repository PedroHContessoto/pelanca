// Ficheiro: src/engine/eval/nnue.rs
// Descrição: Avaliador NNUE v3 — cache externo + atualização incremental.
//
// Board permanece leve (~88 bytes). Acumuladores ficam num cache hash table
// no evaluator, indexados por zobrist hash. update_eval_state() faz a
// atualização incremental (~2-4 features) e guarda no cache.
// evaluate() lê do cache e faz o forward pass.

use crate::core::board::Board;
use crate::core::types::{Bitboard, Color, Move, PieceKind};
use crate::engine::traits::{Evaluator, Score};
use std::sync::Arc;

const INPUT_SIZE: usize = 768;
const FT_SIZE: usize = 256;
const HIDDEN_SIZE: usize = 8; // v2: reduzido de 32→8 (4x menos compute no gargalo)

const FT_QUANT_SCALE: i32 = 64;
const HIDDEN_QUANT_SCALE: i32 = 64;
const OUTPUT_QUANT_SCALE: i32 = 64;
const CRELU_MAX_FT: i16 = FT_QUANT_SCALE as i16;
const CRELU_MAX_HIDDEN: i32 = (FT_QUANT_SCALE * HIDDEN_QUANT_SCALE) as i32;

const MAGIC: [u8; 4] = *b"PLNN";
const VERSION: u32 = 2; // v2: hidden=8

// Cache: potência de 2 para masking rápido
const ACC_CACHE_BITS: usize = 17;  // 2^17 = 131072 entradas
const ACC_CACHE_SIZE: usize = 1 << ACC_CACHE_BITS;
const ACC_CACHE_MASK: usize = ACC_CACHE_SIZE - 1;

// ============================================================================
// PESOS
// ============================================================================

pub struct NnueWeights {
    ft_weight_t: Vec<i16>,  // [INPUT_SIZE][FT_SIZE] transposto, contíguo
    ft_bias: Vec<i16>,      // [FT_SIZE]
    hidden_weight: Vec<i16>, // i8→i16 expandido no load (auto-vectoriza com pmaddwd)
    hidden_bias: Vec<i32>,   // [HIDDEN_SIZE]
    output_weight: Vec<i16>, // i8→i16 expandido
    output_bias: i32,
}

impl NnueWeights {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut o = 0;
        if data.len() < 20 { return Err("NNUE file too small".into()); }
        if &data[0..4] != &MAGIC { return Err("Bad magic".into()); }
        o += 4;
        let ver = read_u32(data, &mut o);
        if ver != VERSION { return Err(format!("Bad version: {}", ver)); }
        let is = read_u32(data, &mut o) as usize;
        let fs = read_u32(data, &mut o) as usize;
        let hs = read_u32(data, &mut o) as usize;
        if is != INPUT_SIZE || fs != FT_SIZE || hs != HIDDEN_SIZE {
            return Err("Architecture mismatch".into());
        }

        let ft_orig = read_i16_vec(data, &mut o, FT_SIZE * INPUT_SIZE);
        let ft_bias = read_i16_vec(data, &mut o, FT_SIZE);
        // Transpor [FT_SIZE][INPUT_SIZE] -> [INPUT_SIZE][FT_SIZE]
        let mut ft_weight_t = vec![0i16; INPUT_SIZE * FT_SIZE];
        for n in 0..FT_SIZE {
            for f in 0..INPUT_SIZE {
                ft_weight_t[f * FT_SIZE + n] = ft_orig[n * INPUT_SIZE + f];
            }
        }

        // Ler i8 e expandir para i16 (permite auto-vectorização com pmaddwd/pmullw)
        let hw_i8 = read_i8_vec(data, &mut o, HIDDEN_SIZE * FT_SIZE * 2);
        let hidden_weight: Vec<i16> = hw_i8.iter().map(|&x| x as i16).collect();
        let hidden_bias = read_i32_vec(data, &mut o, HIDDEN_SIZE);
        let ow_i8 = read_i8_vec(data, &mut o, HIDDEN_SIZE);
        let output_weight: Vec<i16> = ow_i8.iter().map(|&x| x as i16).collect();
        let output_bias = read_i32_single(data, &mut o);

        Ok(NnueWeights { ft_weight_t, ft_bias, hidden_weight, hidden_bias, output_weight, output_bias })
    }

    #[inline(always)]
    fn ft_column(&self, feat: usize) -> &[i16] {
        let s = feat * FT_SIZE;
        &self.ft_weight_t[s..s + FT_SIZE]
    }
}

// ============================================================================
// CACHE DE ACUMULADORES (interior mutability via UnsafeCell)
// ============================================================================

struct AccEntry {
    hash: u64,
    acc: [[i16; FT_SIZE]; 2], // [0]=White persp, [1]=Black persp
}

struct AccCache {
    entries: std::cell::UnsafeCell<Vec<AccEntry>>,
}

unsafe impl Send for AccCache {}
unsafe impl Sync for AccCache {}

impl AccCache {
    fn new() -> Self {
        let mut v = Vec::with_capacity(ACC_CACHE_SIZE);
        for _ in 0..ACC_CACHE_SIZE {
            v.push(AccEntry { hash: 0, acc: [[0i16; FT_SIZE]; 2] });
        }
        AccCache { entries: std::cell::UnsafeCell::new(v) }
    }

    #[inline(always)]
    fn get(&self, hash: u64) -> Option<&[[i16; FT_SIZE]; 2]> {
        let entries = unsafe { &*self.entries.get() };
        let e = &entries[(hash as usize) & ACC_CACHE_MASK];
        if e.hash == hash { Some(&e.acc) } else { None }
    }

    #[inline(always)]
    fn put(&self, hash: u64, acc: &[[i16; FT_SIZE]; 2]) {
        let entries = unsafe { &mut *self.entries.get() };
        let e = &mut entries[(hash as usize) & ACC_CACHE_MASK];
        e.hash = hash;
        e.acc = *acc;
    }
}

// ============================================================================
// AVALIADOR NNUE
// ============================================================================

pub struct NnueEvaluator {
    weights: Arc<NnueWeights>,
    cache: AccCache,
}

impl NnueEvaluator {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let weights = NnueWeights::from_bytes(data)?;
        Ok(NnueEvaluator { weights: Arc::new(weights), cache: AccCache::new() })
    }

    /// Computa acumulador do zero.
    fn compute_acc(&self, board: &Board, persp: Color) -> [i16; FT_SIZE] {
        let w = &self.weights;
        let mut acc = [0i16; FT_SIZE];
        acc.copy_from_slice(&w.ft_bias);
        for &(bb, kind) in &[
            (board.pawns, PieceKind::Pawn), (board.knights, PieceKind::Knight),
            (board.bishops, PieceKind::Bishop), (board.rooks, PieceKind::Rook),
            (board.queens, PieceKind::Queen), (board.kings, PieceKind::King),
        ] {
            add_pieces(&mut acc, bb & board.white_pieces, Color::White, kind, persp, w);
            add_pieces(&mut acc, bb & board.black_pieces, Color::Black, kind, persp, w);
        }
        acc
    }

    /// Computa ambos os acumuladores e guarda no cache.
    fn compute_and_cache(&self, board: &Board) -> [[i16; FT_SIZE]; 2] {
        let acc = [
            self.compute_acc(board, Color::White),
            self.compute_acc(board, Color::Black),
        ];
        self.cache.put(board.zobrist_hash, &acc);
        acc
    }

    /// Forward pass: i16×i16 → i32, loop limpo para auto-vectorização.
    #[inline(always)]
    fn forward(&self, stm_acc: &[i16; FT_SIZE], nstm_acc: &[i16; FT_SIZE]) -> Score {
        let w = &self.weights;

        // ClippedReLU → i16 concatenado [STM | NSTM]
        let mut input = [0i16; FT_SIZE * 2];
        for i in 0..FT_SIZE { input[i] = stm_acc[i].max(0).min(CRELU_MAX_FT); }
        for i in 0..FT_SIZE { input[FT_SIZE + i] = nstm_acc[i].max(0).min(CRELU_MAX_FT); }

        // Hidden layer: loop simples, o compilador com LTO emite pmaddwd
        let mut hidden = [0i32; HIDDEN_SIZE];
        for i in 0..HIDDEN_SIZE {
            let row = &w.hidden_weight[i * (FT_SIZE * 2)..(i + 1) * (FT_SIZE * 2)];
            let mut sum = w.hidden_bias[i];
            for j in 0..(FT_SIZE * 2) {
                sum += row[j] as i32 * input[j] as i32;
            }
            hidden[i] = sum.max(0).min(CRELU_MAX_HIDDEN);
        }

        // Output layer
        let mut out = w.output_bias as i64;
        for i in 0..HIDDEN_SIZE {
            out += w.output_weight[i] as i64 * hidden[i] as i64;
        }
        let scale = (FT_QUANT_SCALE as i64) * (HIDDEN_QUANT_SCALE as i64) * (OUTPUT_QUANT_SCALE as i64);
        ((out * 400) / scale) as Score
    }
}

/// Contagem rápida de material (centipawns, perspectiva do STM).
#[inline]
fn quick_material(board: &Board) -> Score {
    const VALUES: [Score; 6] = [100, 320, 330, 500, 900, 0]; // P N B R Q K
    let bbs = [board.pawns, board.knights, board.bishops, board.rooks, board.queens, board.kings];
    let mut white = 0i32;
    let mut black = 0i32;
    for i in 0..5 { // ignorar rei
        white += (bbs[i] & board.white_pieces).count_ones() as i32 * VALUES[i];
        black += (bbs[i] & board.black_pieces).count_ones() as i32 * VALUES[i];
    }
    match board.to_move {
        Color::White => white - black,
        Color::Black => black - white,
    }
}

impl Evaluator for NnueEvaluator {
    fn evaluate(&self, board: &Board) -> Score {
        // Tentar cache
        let acc = match self.cache.get(board.zobrist_hash) {
            Some(a) => a,
            None => {
                let _ = self.compute_and_cache(board);
                self.cache.get(board.zobrist_hash).unwrap()
            }
        };

        let (stm, nstm) = match board.to_move {
            Color::White => (&acc[0], &acc[1]),
            Color::Black => (&acc[1], &acc[0]),
        };
        let nnue_score = self.forward(stm, nstm);

        // Material anchor: misturar NNUE (80%) com material (20%)
        // Evita sacrifícios posicionais insensatos quando a busca é rasa
        let mat = quick_material(board);
        (nnue_score * 4 + mat) / 5
    }

    fn name(&self) -> &str { "nnue-v3" }

    fn init_eval_state(&self, board: &mut Board) {
        self.compute_and_cache(board);
    }

    fn update_eval_state(&self, parent: &Board, child: &mut Board, mv: Move) {
        // Buscar acumuladores do parent no cache
        let parent_acc = match self.cache.get(parent.zobrist_hash) {
            Some(a) => *a, // copiar (o cache pode ser sobrescrito)
            None => {
                // Parent não está no cache — computar child do zero
                self.compute_and_cache(child);
                return;
            }
        };

        let mut child_acc = parent_acc; // começar com acumuladores do parent
        let moving_color = parent.to_move;
        let w = &self.weights;

        // Identificar peça movida e capturada
        let moved_piece = match parent.get_piece_at(mv.from) {
            Some(p) => p.kind,
            None => { self.compute_and_cache(child); return; }
        };

        let captured: Option<(PieceKind, u8)> = if mv.is_en_passant {
            let sq = if moving_color == Color::White { mv.to - 8 } else { mv.to + 8 };
            Some((PieceKind::Pawn, sq))
        } else {
            parent.get_piece_at(mv.to).map(|p| (p.kind, mv.to))
        };

        // Atualizar ambas perspectivas incrementalmente
        for pi in 0..2usize {
            let persp = if pi == 0 { Color::White } else { Color::Black };
            let acc = &mut child_acc[pi];

            // Remover peça da origem
            sub_col(acc, w.ft_column(feat_idx(moving_color, moved_piece, mv.from, persp)));
            // Adicionar peça ao destino (ou promovida)
            let dest = mv.promotion.unwrap_or(moved_piece);
            add_col(acc, w.ft_column(feat_idx(moving_color, dest, mv.to, persp)));
            // Captura
            if let Some((ck, cs)) = captured {
                sub_col(acc, w.ft_column(feat_idx(!moving_color, ck, cs, persp)));
            }
            // Roque: mover torre
            if mv.is_castling {
                let (rf, rt) = castling_rook_sqs(moving_color, mv.to);
                sub_col(acc, w.ft_column(feat_idx(moving_color, PieceKind::Rook, rf, persp)));
                add_col(acc, w.ft_column(feat_idx(moving_color, PieceKind::Rook, rt, persp)));
            }
        }

        self.cache.put(child.zobrist_hash, &child_acc);
    }
}

// ============================================================================
// HELPERS
// ============================================================================

#[inline(always)]
fn add_pieces(acc: &mut [i16; FT_SIZE], mut bb: Bitboard, color: Color, kind: PieceKind, persp: Color, w: &NnueWeights) {
    while bb != 0 {
        let sq = bb.trailing_zeros() as u8;
        bb &= bb - 1;
        add_col(acc, w.ft_column(feat_idx(color, kind, sq, persp)));
    }
}

#[inline(always)]
fn add_col(acc: &mut [i16; FT_SIZE], col: &[i16]) {
    for i in 0..FT_SIZE { acc[i] = acc[i].wrapping_add(col[i]); }
}

#[inline(always)]
fn sub_col(acc: &mut [i16; FT_SIZE], col: &[i16]) {
    for i in 0..FT_SIZE { acc[i] = acc[i].wrapping_sub(col[i]); }
}

#[inline(always)]
fn feat_idx(color: Color, kind: PieceKind, sq: u8, persp: Color) -> usize {
    let (c, s) = match persp {
        Color::White => (match color { Color::White => 0, Color::Black => 1 }, sq as usize),
        Color::Black => (match color { Color::White => 1, Color::Black => 0 }, (sq ^ 56) as usize),
    };
    let p = match kind {
        PieceKind::Pawn => 0, PieceKind::Knight => 1, PieceKind::Bishop => 2,
        PieceKind::Rook => 3, PieceKind::Queen => 4, PieceKind::King => 5,
    };
    c * 384 + p * 64 + s
}

#[inline(always)]
fn castling_rook_sqs(color: Color, king_to: u8) -> (u8, u8) {
    match color {
        Color::White => if king_to == 6 { (7, 5) } else { (0, 3) },
        Color::Black => if king_to == 62 { (63, 61) } else { (56, 59) },
    }
}


// ============================================================================
// LEITURA DE BYTES
// ============================================================================

#[inline] fn read_u32(d: &[u8], o: &mut usize) -> u32 { let v = u32::from_le_bytes([d[*o],d[*o+1],d[*o+2],d[*o+3]]); *o+=4; v }
#[inline] fn read_i16_vec(d: &[u8], o: &mut usize, n: usize) -> Vec<i16> { let mut v = Vec::with_capacity(n); for _ in 0..n { v.push(i16::from_le_bytes([d[*o],d[*o+1]])); *o+=2; } v }
#[inline] fn read_i8_vec(d: &[u8], o: &mut usize, n: usize) -> Vec<i8> { let mut v = Vec::with_capacity(n); for _ in 0..n { v.push(d[*o] as i8); *o+=1; } v }
#[inline] fn read_i32_vec(d: &[u8], o: &mut usize, n: usize) -> Vec<i32> { let mut v = Vec::with_capacity(n); for _ in 0..n { v.push(i32::from_le_bytes([d[*o],d[*o+1],d[*o+2],d[*o+3]])); *o+=4; } v }
#[inline] fn read_i32_single(d: &[u8], o: &mut usize) -> i32 { let v = i32::from_le_bytes([d[*o],d[*o+1],d[*o+2],d[*o+3]]); *o+=4; v }
