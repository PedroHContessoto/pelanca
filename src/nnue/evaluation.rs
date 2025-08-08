// Sistema de avaliação NNUE integrado ao Board do Pelanca
// Usa zobrist hash para cache e make/unmake para eficiência máxima

use crate::core::{Board, Move, UndoInfo};
use super::{NNUE, NNUEAccumulator};
use std::collections::HashMap;

/// Cache de avaliações NNUE usando zobrist hash
pub struct NNUEEvaluationCache {
    cache: HashMap<u64, i32>,
    hits: u64,
    misses: u64,
}

impl NNUEEvaluationCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(1024 * 1024), // 1M entradas
            hits: 0,
            misses: 0,
        }
    }
    
    pub fn get(&mut self, hash: u64) -> Option<i32> {
        if let Some(&eval) = self.cache.get(&hash) {
            self.hits += 1;
            Some(eval)
        } else {
            self.misses += 1;
            None
        }
    }
    
    pub fn insert(&mut self, hash: u64, eval: i32) {
        self.cache.insert(hash, eval);
    }
    
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }
    
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

/// Integração NNUE com o Board existente
impl Board {
    /// Avalia posição usando NNUE com cache zobrist
    pub fn evaluate_nnue(&self, nnue: &NNUE, cache: &mut NNUEEvaluationCache) -> i32 {
        // Verifica cache primeiro
        if let Some(cached_eval) = cache.get(self.zobrist_hash) {
            return cached_eval;
        }
        
        // Calcula avaliação
        let mut accumulator = NNUEAccumulator::new();
        accumulator.refresh_full(nnue, self);
        let eval = nnue.evaluate_incremental(&accumulator);
        
        // Ajusta pela perspectiva do jogador atual e normaliza melhor
        let normalized_eval = (eval * 100) / 400; // Nova normalização
        let final_eval = if self.to_move == crate::core::Color::White { normalized_eval } else { -normalized_eval };
        
        // Salva no cache
        cache.insert(self.zobrist_hash, final_eval);
        final_eval
    }
    
    /// Avaliação NNUE rápida sem cache (para performance crítica)
    pub fn evaluate_nnue_fast(&self, nnue: &NNUE, accumulator: &mut NNUEAccumulator) -> i32 {
        // Atualiza accumulator se necessário
        if accumulator.needs_refresh || accumulator.cached_hash != self.zobrist_hash {
            accumulator.refresh_full(nnue, self);
        }
        
        let eval = nnue.evaluate_incremental(accumulator);
        let normalized_eval = (eval * 100) / 400; // Nova normalização
        
        // Ajusta pela perspectiva
        if self.to_move == crate::core::Color::White { normalized_eval } else { -normalized_eval }
    }
    
    /// Evaluate com update incremental (para busca com make/unmake)
    pub fn evaluate_nnue_incremental(
        &self,
        nnue: &NNUE,
        accumulator: &mut NNUEAccumulator,
        mv: Move,
        undo_info: &UndoInfo
    ) -> i32 {
        // Atualiza features incrementalmente
        accumulator.update_move(nnue, mv, undo_info, self);
        
        let eval = nnue.evaluate_incremental(accumulator);
        let normalized_eval = (eval * 100) / 400; // Nova normalização
        
        // Ajusta pela perspectiva
        if self.to_move == crate::core::Color::White { normalized_eval } else { -normalized_eval }
    }
}

/// Contexto de avaliação NNUE para uso em buscas
pub struct NNUEContext {
    pub nnue: NNUE,
    pub cache: NNUEEvaluationCache,
    pub accumulator: NNUEAccumulator,
}

impl NNUEContext {
    pub fn new() -> Self {
        Self {
            nnue: NNUE::new(),
            cache: NNUEEvaluationCache::new(),
            accumulator: NNUEAccumulator::new(),
        }
    }
    
    pub fn from_file(path: &str) -> std::io::Result<Self> {
        Ok(Self {
            nnue: NNUE::load(path)?,
            cache: NNUEEvaluationCache::new(),
            accumulator: NNUEAccumulator::new(),
        })
    }
    
    /// Avalia posição com melhor método baseado no contexto
    pub fn evaluate(&mut self, board: &Board) -> i32 {
        board.evaluate_nnue(&self.nnue, &mut self.cache)
    }
    
    /// Avalia com update incremental (para busca)
    pub fn evaluate_after_move(&mut self, board: &Board, mv: Move, undo_info: &UndoInfo) -> i32 {
        board.evaluate_nnue_incremental(&self.nnue, &mut self.accumulator, mv, undo_info)
    }
    
    /// Desfaz avaliação (para busca com unmake)
    pub fn undo_evaluation(&mut self, mv: Move, undo_info: &UndoInfo) {
        self.accumulator.undo_move(&self.nnue, mv, undo_info);
    }
    
    /// Reset para nova linha de busca
    pub fn reset_for_new_search(&mut self) {
        self.accumulator.needs_refresh = true;
    }
    
    /// Estatísticas do cache
    pub fn cache_stats(&self) -> (u64, u64, f64) {
        (self.cache.hits, self.cache.misses, self.cache.hit_rate())
    }
}

impl Default for NNUEContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper para benchmarks e testes
pub fn benchmark_nnue_evaluation(board: &Board, nnue: &NNUE, iterations: usize) -> (std::time::Duration, f64) {
    use std::time::Instant;
    
    let mut cache = NNUEEvaluationCache::new();
    let start = Instant::now();
    
    for _ in 0..iterations {
        board.evaluate_nnue(nnue, &mut cache);
    }
    
    let elapsed = start.elapsed();
    let evals_per_sec = iterations as f64 / elapsed.as_secs_f64();
    
    (elapsed, evals_per_sec)
}

/// Teste de corretude dos updates incrementais
pub fn test_incremental_correctness(board: &mut Board, nnue: &NNUE) -> bool {
    let moves = board.generate_legal_moves();
    if moves.is_empty() {
        return true;
    }
    
    let mut accumulator = NNUEAccumulator::new();
    accumulator.refresh_full(nnue, board);
    let _eval_before = nnue.evaluate_incremental(&accumulator);
    
    for &mv in &moves[..5.min(moves.len())] { // Testa primeiros 5 movimentos
        let undo_info = board.make_move_with_undo(mv);
        
        // Avaliação incremental
        accumulator.update_move(nnue, mv, &undo_info, board);
        let eval_incremental = nnue.evaluate_incremental(&accumulator);
        
        // Avaliação completa para comparar
        let mut fresh_accumulator = NNUEAccumulator::new();
        fresh_accumulator.refresh_full(nnue, board);
        let eval_fresh = nnue.evaluate_incremental(&fresh_accumulator);
        
        board.unmake_move(mv, undo_info);
        accumulator.undo_move(nnue, mv, &undo_info);
        
        // Verifica se são iguais
        if (eval_incremental - eval_fresh).abs() > 1 { // Tolerância para arredondamento
            println!("Erro incremental: mv={}, inc={}, fresh={}", mv, eval_incremental, eval_fresh);
            return false;
        }
    }
    
    true
}