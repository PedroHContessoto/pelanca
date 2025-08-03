use fxhash::FxHashMap as HashMap;
use crate::core::Move;

/// Entry para Transposition Table de busca Alpha-Beta
#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub score: i32,
    pub flag: u8,      // 0=EXACT, 1=ALPHA, 2=BETA
    pub depth: u8,
    pub best_move: Option<Move>,
}

/// Flags para tipos de entrada na TT
pub const TT_EXACT: u8 = 0;
pub const TT_ALPHA: u8 = 1;
pub const TT_BETA: u8 = 2;

/// Transposition Table otimizada para Alpha-Beta com FxHash (ultra-rápido)
pub struct TranspositionTable {
    table: HashMap<u64, TTEntry>, // zobrist_hash -> TTEntry (FxHash)
    hits: u64,
    misses: u64,
    max_capacity: usize,
}

impl TranspositionTable {
    pub fn new() -> Self {
        Self::with_capacity(2_000_000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        TranspositionTable {
            table: HashMap::with_capacity_and_hasher(capacity, Default::default()),
            hits: 0,
            misses: 0,
            max_capacity: capacity,
        }
    }
    
    /// Busca entrada na TT
    pub fn probe(&mut self, hash: u64, depth: u8, alpha: i32, beta: i32) -> Option<i32> {
        if let Some(&entry) = self.table.get(&hash) {
            self.hits += 1;
            
            // Só usa se a profundidade for igual ou maior
            if entry.depth >= depth {
                match entry.flag {
                    TT_EXACT => return Some(entry.score),
                    TT_ALPHA if entry.score <= alpha => return Some(alpha),
                    TT_BETA if entry.score >= beta => return Some(beta),
                    _ => {}
                }
            }
            None
        } else {
            self.misses += 1;
            None
        }
    }
    
    /// Armazena entrada na TT
    pub fn store(&mut self, hash: u64, depth: u8, score: i32, flag: u8, best_move: Option<Move>) {
        // Evicção simples: remove entradas antigas se atingir 90% da capacidade
        if self.table.len() >= (self.max_capacity * 9) / 10 {
            self.clear_old_entries();
        }
        
        let entry = TTEntry {
            score,
            flag,
            depth,
            best_move,
        };
        
        // Always replace ou depth-preferred replacement
        if let Some(&existing) = self.table.get(&hash) {
            if depth >= existing.depth {
                self.table.insert(hash, entry);
            }
        } else {
            self.table.insert(hash, entry);
        }
    }
    
    /// Busca melhor movimento da TT
    pub fn get_best_move(&mut self, hash: u64) -> Option<Move> {
        if let Some(&entry) = self.table.get(&hash) {
            entry.best_move
        } else {
            None
        }
    }
    
    /// Remove entradas antigas (estratégia profissional: LRU aproximado)
    fn clear_old_entries(&mut self) {
        // Remove entradas com profundidade baixa primeiro (LRU aproximado)
        let mut to_remove = Vec::new();
        
        // Primeiro passo: remove entradas com depth < 5
        for (&hash, entry) in &self.table {
            if entry.depth < 5 {
                to_remove.push(hash);
            }
        }
        
        // Se ainda precisar remover mais, remove por depth baixa
        if to_remove.len() < self.table.len() / 4 {
            let mut entries_by_depth: Vec<(u64, u8)> = self.table
                .iter()
                .map(|(&hash, entry)| (hash, entry.depth))
                .collect();
            
            // Ordena por profundidade (menores primeiro)
            entries_by_depth.sort_by_key(|(_, depth)| *depth);
            
            // Remove 25% das entradas com menor profundidade
            let remove_count = self.table.len() / 4;
            for (hash, _) in entries_by_depth.into_iter().take(remove_count) {
                to_remove.push(hash);
            }
        }
        
        // Remove as entradas selecionadas
        for hash in to_remove {
            self.table.remove(&hash);
        }
    }
    
    /// Limpa toda a TT
    pub fn clear(&mut self) {
        self.table.clear();
        self.hits = 0;
        self.misses = 0;
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
    
    pub fn size(&self) -> usize {
        self.table.len()
    }
    
    pub fn capacity(&self) -> usize {
        self.max_capacity
    }
    
    /// Hashfull: percentual de ocupação da TT (0-1000)
    pub fn hashfull(&self) -> u64 {
        (self.size() as u64 * 1000) / self.capacity() as u64
    }
}