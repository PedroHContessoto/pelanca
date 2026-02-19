// Ficheiro: src/engine/search/tt.rs
// Descrição: Transposition Table para busca alpha-beta.
// Tabela hash de tamanho fixo com entradas compactas de 12 bytes.

use crate::core::types::{Move, PieceKind};

/// Tipo de bound armazenado no TT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Bound {
    /// Score exato (nó PV: alpha < score < beta)
    Exact = 0,
    /// Lower bound (fail-high: score >= beta)
    Lower = 1,
    /// Upper bound (fail-low: score <= alpha)
    Upper = 2,
}

/// Entrada da transposition table — 12 bytes.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct TTEntry {
    pub key: u32,
    pub best_move: u16,
    pub score: i16,
    pub depth: u8,
    pub bound: u8,
    pub age: u8,
    _pad: u8,
}

impl TTEntry {
    const EMPTY: Self = TTEntry {
        key: 0,
        best_move: 0,
        score: 0,
        depth: 0,
        bound: 0,
        age: 0,
        _pad: 0,
    };

    pub fn bound_type(&self) -> Bound {
        match self.bound {
            1 => Bound::Lower,
            2 => Bound::Upper,
            _ => Bound::Exact,
        }
    }
}

/// Empacota um Move em 16 bits: from(6) | to(6) | promo(3) | flags(1)
/// flags bit: 0 = castling, 1 = en passant (mutuamente exclusivos)
pub fn pack_move(mv: Move) -> u16 {
    let from = (mv.from & 0x3F) as u16;
    let to = (mv.to & 0x3F) as u16;
    let promo = match mv.promotion {
        None => 0u16,
        Some(PieceKind::Knight) => 1,
        Some(PieceKind::Bishop) => 2,
        Some(PieceKind::Rook) => 3,
        Some(PieceKind::Queen) => 4,
        _ => 0,
    };
    // Bit 15 = is_castling ou is_en_passant (distinguidos pela geometria)
    let special = if mv.is_castling || mv.is_en_passant { 1u16 } else { 0 };
    from | (to << 6) | (promo << 12) | (special << 15)
}

/// Desempacota 16 bits para Move. Retorna None se packed == 0.
/// Castling vs en passant é deduzido pela geometria do lance:
///   - Castling: rei move 2 casas (from=4/60, |to-from|=2)
///   - En passant: peão captura diagonal (diferença de file=1, special bit set, no promo)
pub fn unpack_move(packed: u16) -> Option<Move> {
    if packed == 0 {
        return None;
    }
    let from = (packed & 0x3F) as u8;
    let to = ((packed >> 6) & 0x3F) as u8;
    let promo_bits = (packed >> 12) & 0x07;
    let promotion = match promo_bits {
        1 => Some(PieceKind::Knight),
        2 => Some(PieceKind::Bishop),
        3 => Some(PieceKind::Rook),
        4 => Some(PieceKind::Queen),
        _ => None,
    };
    let special = (packed >> 15) & 1;

    let mut is_castling = false;
    let mut is_en_passant = false;

    if special != 0 {
        // Castling: king from e1(4) or e8(60), moves 2 squares
        if (from == 4 || from == 60) && ((to as i8 - from as i8).abs() == 2) {
            is_castling = true;
        } else {
            is_en_passant = true;
        }
    }

    Some(Move {
        from,
        to,
        promotion,
        is_castling,
        is_en_passant,
    })
}

/// Transposition Table com tamanho fixo, indexação por mask.
pub struct TranspositionTable {
    table: Vec<TTEntry>,
    mask: usize,
    generation: u8,
}

impl TranspositionTable {
    /// Cria TT com tamanho em megabytes.
    pub fn new(mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TTEntry>();
        let num_entries = (mb * 1024 * 1024) / entry_size;
        // Arredondar para potência de 2
        let num_entries = num_entries.next_power_of_two() >> 1;
        let num_entries = num_entries.max(1024); // mínimo 1024 entradas

        TranspositionTable {
            table: vec![TTEntry::EMPTY; num_entries],
            mask: num_entries - 1,
            generation: 0,
        }
    }

    /// Índice na tabela via hash.
    #[inline(always)]
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Probe: retorna entrada se key bater.
    #[inline]
    pub fn probe(&self, hash: u64) -> Option<&TTEntry> {
        let idx = self.index(hash);
        let entry = &self.table[idx];
        if entry.depth > 0 && entry.key == (hash >> 32) as u32 {
            Some(entry)
        } else {
            None
        }
    }

    /// Store: guarda entrada com política de replacement.
    #[inline]
    pub fn store(
        &mut self,
        hash: u64,
        depth: u8,
        score: i16,
        bound: Bound,
        best_move: u16,
    ) {
        let idx = self.index(hash);
        let key32 = (hash >> 32) as u32;
        let existing = &self.table[idx];

        // Replace se: mesma posição, depth maior ou igual, ou entrada velha
        if existing.depth == 0
            || existing.key == key32
            || depth >= existing.depth
            || existing.age != self.generation
        {
            self.table[idx] = TTEntry {
                key: key32,
                best_move,
                score,
                depth,
                bound: bound as u8,
                age: self.generation,
                _pad: 0,
            };
        }
    }

    /// Incrementa geração (chamar antes de cada busca nova do ID).
    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Limpa toda a tabela.
    pub fn clear(&mut self) {
        self.table.fill(TTEntry::EMPTY);
        self.generation = 0;
    }

    /// Resize para novo tamanho em MB.
    pub fn resize(&mut self, mb: usize) {
        let entry_size = std::mem::size_of::<TTEntry>();
        let num_entries = (mb * 1024 * 1024) / entry_size;
        let num_entries = num_entries.next_power_of_two() >> 1;
        let num_entries = num_entries.max(1024);

        self.table = vec![TTEntry::EMPTY; num_entries];
        self.mask = num_entries - 1;
        self.generation = 0;
    }

    /// Hashfull: proporção de entradas usadas da geração atual (0-1000 para UCI).
    pub fn hashfull(&self) -> u32 {
        let sample = self.table.len().min(1000);
        let used = self.table[..sample]
            .iter()
            .filter(|e| e.depth > 0 && e.age == self.generation)
            .count();
        (used as u32 * 1000) / sample as u32
    }
}
