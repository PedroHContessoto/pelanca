use std::collections::HashMap;

/// Transposition Table para cache de resultados perft
pub struct PerftTT {
    table: HashMap<(u64, u8), u64>, // (zobrist_hash, depth) -> nodes
    hits: u64,
    misses: u64,
}

impl PerftTT {
    pub fn new() -> Self {
        PerftTT {
            table: HashMap::with_capacity(2_000_000), // ~16MB cache
            hits: 0,
            misses: 0,
        }
    }
    
    pub fn get(&mut self, hash: u64, depth: u8) -> Option<u64> {
        if let Some(&nodes) = self.table.get(&(hash, depth)) {
            self.hits += 1;
            Some(nodes)
        } else {
            self.misses += 1;
            None
        }
    }
    
    pub fn insert(&mut self, hash: u64, depth: u8, nodes: u64) {
        self.table.insert((hash, depth), nodes);
    }
    
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 { 0.0 }
        else { self.hits as f64 / (self.hits + self.misses) as f64 }
    }
    
    pub fn hits(&self) -> u64 {
        self.hits
    }
    
    pub fn misses(&self) -> u64 {
        self.misses
    }
}