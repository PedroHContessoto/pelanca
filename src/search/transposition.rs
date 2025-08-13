use std::collections::HashMap;
use crate::core::*;
use super::{Score, Depth};

// Tipos de entrada na TT
#[derive(Debug, Clone, Copy)]
pub enum TTNodeType {
    Exact,    // Valor exato (PV-node)
    Alpha,    // Upper bound (All-node)
    Beta,     // Lower bound (Cut-node)
}

// Entrada da Transposition Table
#[derive(Debug, Clone, Copy)]
pub struct TTEntry {
    pub zobrist_hash: u64,
    pub depth: Depth,
    pub score: Score,
    pub node_type: TTNodeType,
    pub best_move: Option<Move>,
    pub age: u8,  // Para replacement scheme
}

/// Transposition Table para cache de posições durante o search
/// Evolução da PerftTT otimizada para jogo competitivo
pub struct TranspositionTable {
    table: HashMap<u64, TTEntry>,
    hits: u64,
    misses: u64,
    current_age: u8,
}

impl TranspositionTable {
    pub fn new() -> Self {
        Self::with_size(16_777_216) // 16MB padrão
    }

    pub fn with_size(size_bytes: usize) -> Self {
        let capacity = size_bytes / std::mem::size_of::<TTEntry>();
        
        Self {
            table: HashMap::with_capacity(capacity),
            hits: 0,
            misses: 0,
            current_age: 0,
        }
    }

    /// Busca uma posição na TT
    pub fn probe(&mut self, hash: u64, depth: Depth, alpha: Score, beta: Score) -> Option<Score> {
        if let Some(entry) = self.table.get(&hash) {
            self.hits += 1;
            
            // Verifica se a profundidade é suficiente
            if entry.depth >= depth {
                match entry.node_type {
                    TTNodeType::Exact => return Some(entry.score),
                    TTNodeType::Alpha if entry.score <= alpha => return Some(alpha),
                    TTNodeType::Beta if entry.score >= beta => return Some(beta),
                    _ => {}
                }
            }
        } else {
            self.misses += 1;
        }
        
        None
    }

    /// Armazena uma posição na TT
    pub fn store(&mut self, hash: u64, depth: Depth, score: Score, 
                 node_type: TTNodeType, best_move: Option<Move>) {
        
        let entry = TTEntry {
            zobrist_hash: hash,
            depth,
            score,
            node_type,
            best_move,
            age: self.current_age,
        };

        // Replacement scheme: always replace (simples)
        // TODO: Implementar depth-preferred replacement
        self.table.insert(hash, entry);
    }

    /// Obtém o melhor movimento de uma posição (para move ordering)
    pub fn get_best_move(&self, hash: u64) -> Option<Move> {
        self.table.get(&hash).and_then(|entry| entry.best_move)
    }

    /// Limpa a TT para novo jogo
    pub fn clear(&mut self) {
        self.table.clear();
        self.hits = 0;
        self.misses = 0;
        self.current_age = 0;
    }

    /// Incrementa idade para aging entries
    pub fn age(&mut self) {
        self.current_age = self.current_age.wrapping_add(1);
    }

    /// Estatísticas da TT
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 }
        else { self.hits as f64 / total as f64 }
    }

    pub fn size(&self) -> usize {
        self.table.len()
    }

    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.capacity() == 0 { 0.0 }
        else { self.size() as f64 / self.capacity() as f64 * 100.0 }
    }

    /// Converte score mate para mate relativo à posição atual
    pub fn score_to_tt(score: Score, ply: u8) -> Score {
        if score >= crate::search::MATE_IN_MAX {
            score + ply as Score
        } else if score <= -crate::search::MATE_IN_MAX {
            score - ply as Score
        } else {
            score
        }
    }

    /// Converte score da TT de volta para score absoluto
    pub fn score_from_tt(score: Score, ply: u8) -> Score {
        if score >= crate::search::MATE_IN_MAX {
            score - ply as Score
        } else if score <= -crate::search::MATE_IN_MAX {
            score + ply as Score
        } else {
            score
        }
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}