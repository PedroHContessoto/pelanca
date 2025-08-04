use crate::core::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::mem;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Exact,
    LowerBound,
    UpperBound,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub data: u64, // Packed: move(16) + score(16) + depth(8) + type(2) + age(6) + padding(16)
}

impl TTEntry {
    pub fn new(hash: u64, best_move: Move, score: i16, depth: u8, node_type: NodeType, age: u8) -> Self {
        let move_bits = ((best_move.from as u64) << 10) | ((best_move.to as u64) << 4) | 
                       (if best_move.is_castling { 1u64 } else { 0u64 }) << 1 |
                       (if best_move.is_en_passant { 1u64 } else { 0u64 });
        let type_bits = match node_type {
            NodeType::Exact => 0u64,
            NodeType::LowerBound => 1u64,
            NodeType::UpperBound => 2u64,
        };
        
        let data = (move_bits << 48) | 
                  ((score as u16 as u64) << 32) |
                  ((depth as u64) << 24) |
                  (type_bits << 22) |
                  ((age as u64) << 16);
                  
        TTEntry { key: hash, data }
    }
    
    pub fn get_move(&self) -> Move {
        let move_bits = self.data >> 48;
        Move {
            from: ((move_bits >> 10) & 0x3F) as u8,
            to: ((move_bits >> 4) & 0x3F) as u8,
            promotion: None,
            is_castling: (move_bits & 0x2) != 0,
            is_en_passant: (move_bits & 0x1) != 0,
        }
    }
    
    pub fn get_score(&self) -> i16 {
        ((self.data >> 32) & 0xFFFF) as u16 as i16
    }
    
    pub fn get_depth(&self) -> u8 {
        ((self.data >> 24) & 0xFF) as u8
    }
    
    pub fn get_type(&self) -> NodeType {
        match (self.data >> 22) & 0x3 {
            0 => NodeType::Exact,
            1 => NodeType::LowerBound,
            _ => NodeType::UpperBound,
        }
    }
    
    pub fn get_age(&self) -> u8 {
        ((self.data >> 16) & 0x3F) as u8
    }
}

impl Default for TTEntry {
    fn default() -> Self {
        TTEntry { key: 0, data: 0 }
    }
}

const BUCKET_SIZE: usize = 4;

#[repr(align(64))]
struct TTBucket {
    entries: [AtomicU64; BUCKET_SIZE * 2], // key, data pairs
}

impl TTBucket {
    fn new() -> Self {
        TTBucket {
            entries: [const { AtomicU64::new(0) }; BUCKET_SIZE * 2],
        }
    }
}

pub struct TranspositionTable {
    buckets: Vec<TTBucket>,
    size: usize,
    mask: usize,
    age: u8,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = mem::size_of::<TTBucket>();
        let target_bytes = size_mb * 1024 * 1024;
        let num_buckets = (target_bytes / entry_size).next_power_of_two();
        
        TranspositionTable {
            buckets: (0..num_buckets).map(|_| TTBucket::new()).collect(),
            size: num_buckets,
            mask: num_buckets - 1,
            age: 0,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn clear(&self) {
        for bucket in &self.buckets {
            for i in 0..BUCKET_SIZE * 2 {
                bucket.entries[i].store(0, Ordering::Relaxed);
            }
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    pub fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    pub fn probe(&self, hash: u64) -> Option<TTEntry> {
        let bucket_idx = (hash as usize) & self.mask;
        let bucket = &self.buckets[bucket_idx];

        for i in 0..BUCKET_SIZE {
            let key = bucket.entries[i * 2].load(Ordering::Acquire);
            if key == hash {
                let data = bucket.entries[i * 2 + 1].load(Ordering::Acquire);
                let entry = TTEntry { key, data };
                
                // Verify entry consistency
                if bucket.entries[i * 2].load(Ordering::Acquire) == key {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(entry);
                }
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    pub fn store(&self, hash: u64, best_move: Move, score: i16, depth: u8, node_type: NodeType) {
        let bucket_idx = (hash as usize) & self.mask;
        let bucket = &self.buckets[bucket_idx];
        
        let new_entry = TTEntry::new(hash, best_move, score, depth, node_type, self.age);

        let mut best_slot = 0;
        let mut best_score = i32::MIN;

        for i in 0..BUCKET_SIZE {
            let key = bucket.entries[i * 2].load(Ordering::Acquire);
            
            if key == 0 || key == hash {
                bucket.entries[i * 2 + 1].store(new_entry.data, Ordering::Release);
                bucket.entries[i * 2].store(new_entry.key, Ordering::Release);
                return;
            }

            let data = bucket.entries[i * 2 + 1].load(Ordering::Acquire);
            let entry = TTEntry { key, data };
            
            let age_bonus = if entry.get_age() == self.age { 0 } else { 100 };
            let depth_penalty = entry.get_depth() as i32;
            let replacement_score = age_bonus - depth_penalty;

            if replacement_score > best_score {
                best_score = replacement_score;
                best_slot = i;
            }
        }

        bucket.entries[best_slot * 2 + 1].store(new_entry.data, Ordering::Release);
        bucket.entries[best_slot * 2].store(new_entry.key, Ordering::Release);
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        
        if total == 0 { 0.0 } else { hits as f64 / total as f64 }
    }

    pub fn usage(&self) -> f64 {
        let mut used = 0;
        let sample_size = self.size.min(1000);
        
        for i in (0..sample_size).step_by(self.size / sample_size) {
            for j in 0..BUCKET_SIZE {
                if self.buckets[i].entries[j * 2].load(Ordering::Relaxed) != 0 {
                    used += 1;
                }
            }
        }
        
        (used as f64) / (sample_size * BUCKET_SIZE) as f64
    }

    pub fn stats(&self) -> (u64, u64, f64, f64) {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        (hits, misses, self.hit_rate(), self.usage())
    }

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