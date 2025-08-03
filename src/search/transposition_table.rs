// Transposition Table de alta performance com substituição inteligente

use crate::core::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Flags para entrada da TT
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TTFlag {
    Exact,      // Score exato
    LowerBound, // Score >= beta (fail-high)
    UpperBound, // Score <= alpha (fail-low)
}

/// Entrada da transposition table (16 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct TTEntry {
    pub key: u32,        // 4 bytes - parte alta do zobrist hash
    pub best_move: Move, // 4 bytes (compactado)
    pub score: i16,      // 2 bytes
    pub depth: u8,       // 1 byte
    pub flag: TTFlag,    // 1 byte
    pub age: u8,         // 1 byte - para substituição
    _padding: [u8; 3],   // 3 bytes - alinhamento para 16 bytes
}

impl TTEntry {
    pub fn new(hash: u64, depth: u8, score: i32, flag: TTFlag, best_move: Move, age: u8) -> Self {
        TTEntry {
            key: (hash >> 32) as u32,
            best_move,
            score: score.clamp(-32000, 32000) as i16,
            depth,
            flag,
            age,
            _padding: [0; 3],
        }
    }

    pub fn is_valid(&self, hash: u64) -> bool {
        self.key == (hash >> 32) as u32
    }
}

/// Transposition Table thread-safe e otimizada
pub struct TranspositionTable {
    table: Vec<AtomicU64>,
    table2: Vec<AtomicU64>, // Segunda metade da entrada (16 bytes total)
    size: usize,
    mask: usize,
    hits: AtomicU64,
    misses: AtomicU64,
    current_age: AtomicU64,
}

impl TranspositionTable {
    /// Cria nova TT com tamanho em MB
    pub fn new(size_mb: usize) -> Self {
        let entry_size = 16; // bytes por entrada
        let num_entries = (size_mb * 1024 * 1024) / entry_size;
        let size = num_entries.next_power_of_two();
        let mask = size - 1;

        TranspositionTable {
            table: (0..size).map(|_| AtomicU64::new(0)).collect(),
            table2: (0..size).map(|_| AtomicU64::new(0)).collect(),
            size,
            mask,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            current_age: AtomicU64::new(0),
        }
    }

    /// Busca entrada na TT
    pub fn probe(&self, hash: u64) -> Option<TTEntry> {
        let index = (hash as usize) & self.mask;

        // Carrega atomicamente os 16 bytes
        let data1 = self.table[index].load(Ordering::Relaxed);
        let data2 = self.table2[index].load(Ordering::Relaxed);

        // Reconstrói a entrada
        let entry = unsafe {
            let mut bytes = [0u8; 17];
            bytes[0..8].copy_from_slice(&data1.to_ne_bytes());
            bytes[8..16].copy_from_slice(&data2.to_ne_bytes());
            std::mem::transmute::<[u8; 17], TTEntry>(bytes)
        };

        if entry.is_valid(hash) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(entry)
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Armazena entrada na TT com política de substituição
    pub fn store(&self, hash: u64, depth: u8, score: i32, flag: TTFlag, best_move: Move) {
        let index = (hash as usize) & self.mask;
        let age = self.current_age.load(Ordering::Relaxed) as u8;

        // Carrega entrada atual para decisão de substituição
        let current_data1 = self.table[index].load(Ordering::Relaxed);
        let current_data2 = self.table2[index].load(Ordering::Relaxed);

        let current_entry = unsafe {
            let mut bytes = [0u8; 17];
            bytes[0..8].copy_from_slice(&current_data1.to_ne_bytes());
            bytes[8..16].copy_from_slice(&current_data2.to_ne_bytes());
            std::mem::transmute::<[u8; 17], TTEntry>(bytes)
        };

        // Política de substituição
        let should_replace = if !current_entry.is_valid(hash) {
            true // Entrada vazia ou hash diferente
        } else if current_entry.age != age {
            true // Entrada antiga
        } else if depth >= current_entry.depth {
            true // Nova busca é mais profunda
        } else {
            false
        };

        if should_replace {
            let new_entry = TTEntry::new(hash, depth, score, flag, best_move, age);

            // Converte para bytes e armazena atomicamente
            let bytes = unsafe {
                std::mem::transmute::<TTEntry, [u8; 17]>(new_entry)
            };

            let data1 = u64::from_ne_bytes(bytes[0..8].try_into().unwrap());
            let data2 = u64::from_ne_bytes(bytes[8..16].try_into().unwrap());

            self.table[index].store(data1, Ordering::Relaxed);
            self.table2[index].store(data2, Ordering::Relaxed);
        }
    }

    /// Limpa a TT
    pub fn clear(&self) {
        for i in 0..self.size {
            self.table[i].store(0, Ordering::Relaxed);
            self.table2[i].store(0, Ordering::Relaxed);
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Incrementa idade para política de substituição
    pub fn new_search(&self) {
        self.current_age.fetch_add(1, Ordering::Relaxed);
    }

    /// Retorna estatísticas (hits, misses)
    pub fn get_stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed)
        )
    }

    /// Taxa de acerto
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        if hits + misses > 0.0 {
            hits / (hits + misses)
        } else {
            0.0
        }
    }

    /// Uso da tabela em porcentagem
    pub fn usage(&self) -> f64 {
        let mut used = 0;
        let sample_size = (self.size / 1000).max(1000).min(self.size);

        for i in 0..sample_size {
            let idx = (i * self.size / sample_size) % self.size;
            if self.table[idx].load(Ordering::Relaxed) != 0 {
                used += 1;
            }
        }

        (used as f64 / sample_size as f64) * 100.0
    }
}

/// Ajusta score de mate para armazenar na TT
pub fn score_to_tt(score: i32, ply: i32) -> i32 {
    if score >= super::MATE_THRESHOLD {
        score + ply
    } else if score <= -super::MATE_THRESHOLD {
        score - ply
    } else {
        score
    }
}

/// Ajusta score de mate ao recuperar da TT
pub fn score_from_tt(score: i32, ply: i32) -> i32 {
    if score >= super::MATE_THRESHOLD {
        score - ply
    } else if score <= -super::MATE_THRESHOLD {
        score + ply
    } else {
        score
    }
}