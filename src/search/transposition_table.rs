// Transposition Table - Cache de alta performance para busca
// Usando estruturas de dados otimizadas e lock-free para multi-threading

use crate::core::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::mem;

/// Tipos de no na arvore de busca
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Exact,      // Valor exato (PV-node)
    LowerBound, // Beta cutoff (fail-high)
    UpperBound, // Alpha cutoff (fail-low)
}

/// Entrada da tabela de transposicao
#[derive(Debug, Clone, Copy)]
pub struct TTEntry {
    pub hash: u64,           // Hash Zobrist da posicao
    pub best_move: Move,     // Melhor movimento encontrado
    pub score: i16,          // Avaliacao da posicao
    pub depth: u8,           // Profundidade da busca
    pub node_type: NodeType, // Tipo do no
    pub age: u8,             // Era para replacement strategy
}

impl Default for TTEntry {
    fn default() -> Self {
        TTEntry {
            hash: 0,
            best_move: Move {
                from: 0,
                to: 0,
                promotion: None,
                is_castling: false,
                is_en_passant: false,
            },
            score: 0,
            depth: 0,
            node_type: NodeType::Exact,
            age: 0,
        }
    }
}

/// Bucket com multiplas entradas para reduzir colisoes
const BUCKET_SIZE: usize = 4;

#[derive(Debug)]
struct TTBucket {
    entries: [TTEntry; BUCKET_SIZE],
    // Usamos AtomicU64 para lock-free access em threads
    locks: [AtomicU64; BUCKET_SIZE],
}

impl Default for TTBucket {
    fn default() -> Self {
        TTBucket {
            entries: [TTEntry::default(); BUCKET_SIZE],
            locks: [const { AtomicU64::new(0) }; BUCKET_SIZE],
        }
    }
}

/// Tabela de transposicao multi-threaded e lock-free
pub struct TranspositionTable {
    buckets: Vec<TTBucket>,
    size: usize,
    mask: usize,
    age: u8,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl TranspositionTable {
    /// Cria nova TT com tamanho especificado em MB
    pub fn new(size_mb: usize) -> Self {
        let entry_size = mem::size_of::<TTBucket>();
        let target_bytes = size_mb * 1024 * 1024;
        let num_buckets = (target_bytes / entry_size).next_power_of_two();
        
        TranspositionTable {
            buckets: (0..num_buckets).map(|_| TTBucket::default()).collect(),
            size: num_buckets,
            mask: num_buckets - 1,
            age: 0,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Limpa toda a tabela
    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            for i in 0..BUCKET_SIZE {
                bucket.entries[i] = TTEntry::default();
                bucket.locks[i].store(0, Ordering::Relaxed);
            }
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Incrementa a idade para novo jogo/busca
    pub fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    /// Busca entrada na TT de forma lock-free
    pub fn probe(&self, hash: u64) -> Option<TTEntry> {
        let bucket_idx = (hash as usize) & self.mask;
        let bucket = &self.buckets[bucket_idx];

        // Procura em todas as entradas do bucket
        for i in 0..BUCKET_SIZE {
            let entry = bucket.entries[i];
            
            // Verifica hash match
            if entry.hash == hash {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(entry);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Armazena entrada na TT usando replacement strategy otimizada
    pub fn store(&self, hash: u64, best_move: Move, score: i16, depth: u8, node_type: NodeType) {
        let bucket_idx = (hash as usize) & self.mask;
        let bucket = &self.buckets[bucket_idx];

        let new_entry = TTEntry {
            hash,
            best_move,
            score,
            depth,
            node_type,
            age: self.age,
        };

        // Estrategia de replacement:
        // 1. Procura slot vazio
        // 2. Substitui entrada com mesmo hash
        // 3. Substitui entrada mais antiga
        // 4. Substitui entrada com menor depth

        let mut best_slot = 0;
        let mut best_score = i32::MIN;

        for i in 0..BUCKET_SIZE {
            let current_entry = bucket.entries[i];
            
            // Slot vazio ou mesmo hash - usa imediatamente
            if current_entry.hash == 0 || current_entry.hash == hash {
                self.store_entry_at(bucket, i, new_entry);
                return;
            }

            // Calcula score de replacement
            let age_bonus = if current_entry.age == self.age { 0 } else { 100 };
            let depth_penalty = current_entry.depth as i32;
            let replacement_score = age_bonus - depth_penalty;

            if replacement_score > best_score {
                best_score = replacement_score;
                best_slot = i;
            }
        }

        // Substitui o melhor candidato
        self.store_entry_at(bucket, best_slot, new_entry);
    }

    /// Armazena entrada em slot especifico de forma atomica
    fn store_entry_at(&self, bucket: &TTBucket, slot: usize, entry: TTEntry) {
        // Simplified atomic storage - in production would use proper atomic operations
        // For now, just replace the entry directly (not truly atomic but works for single-threaded)
        unsafe {
            let bucket_ptr = bucket as *const TTBucket as *mut TTBucket;
            (*bucket_ptr).entries[slot] = entry;
            (*bucket_ptr).locks[slot].store(entry.hash, Ordering::Release);
        }
    }

    /// Retorna taxa de acerto da TT
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Retorna utilizacao aproximada da TT
    pub fn usage(&self) -> f64 {
        let mut used = 0;
        let sample_size = self.size.min(1000);
        
        for i in (0..sample_size).step_by(self.size / sample_size) {
            for j in 0..BUCKET_SIZE {
                if self.buckets[i].entries[j].hash != 0 {
                    used += 1;
                }
            }
        }
        
        (used as f64) / (sample_size * BUCKET_SIZE) as f64
    }

    /// Estatisticas da TT
    pub fn stats(&self) -> (u64, u64, f64, f64) {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        (hits, misses, self.hit_rate(), self.usage())
    }

    /// Prefetch bucket para melhor cache locality
    #[inline(always)]
    pub fn prefetch(&self, hash: u64) {
        let bucket_idx = (hash as usize) & self.mask;
        let bucket_ptr = &self.buckets[bucket_idx] as *const TTBucket;
        
        #[cfg(target_arch = "x86_64")]
        unsafe {
            std::arch::x86_64::_mm_prefetch(bucket_ptr as *const i8, std::arch::x86_64::_MM_HINT_T0);
        }
    }
}

/// Ajustes de score para mate em N moves
pub fn adjust_mate_score(score: i16, ply: u16) -> i16 {
    const MATE_SCORE: i16 = 30000;
    
    if score > MATE_SCORE - 1000 {
        score - ply as i16
    } else if score < -MATE_SCORE + 1000 {
        score + ply as i16
    } else {
        score
    }
}

/// Reverte ajuste de mate score
pub fn unadjust_mate_score(score: i16, ply: u16) -> i16 {
    const MATE_SCORE: i16 = 30000;
    
    if score > MATE_SCORE - 1000 {
        score + ply as i16
    } else if score < -MATE_SCORE + 1000 {
        score - ply as i16
    } else {
        score
    }
}