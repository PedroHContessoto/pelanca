use crate::core::*;

/// Late Move Reduction (LMR) - Técnica para reduzir drasticamente o número de nós
/// Reduz a profundidade de busca para movimentos menos promissores
pub struct LateMovePruner;

impl LateMovePruner {
    /// Calcula a redução de profundidade baseada na posição do movimento
    pub fn get_reduction(move_index: usize, depth: u8, is_tactical: bool, is_pv_node: bool) -> u8 {
        // Nunca reduz movimentos táticos ou em nós PV
        if is_tactical || is_pv_node || depth < 3 {
            return 0;
        }
        
        // REDUÇÃO AGRESSIVA para chegar ao depth 17
        match move_index {
            0..=2 => 0,     // Primeiros 3 movimentos - sem redução
            3..=5 => 1,     // Movimentos 4-6 - reduz 1 nível
            6..=10 => 2,    // Movimentos 7-11 - reduz 2 níveis  
            11..=15 => 3,   // Movimentos 12-16 - reduz 3 níveis
            _ => 4,         // Movimentos tardios - reduz 4 níveis (muito agressivo)
        }
    }
    
    /// Calcula redução adaptativa baseada na profundidade atual
    pub fn get_adaptive_reduction(move_index: usize, depth: u8, is_tactical: bool) -> u8 {
        if is_tactical || depth < 3 {
            return 0;
        }
        
        // Mais agressivo em profundidades altas
        let base_reduction = Self::get_reduction(move_index, depth, is_tactical, false);
        
        if depth >= 8 {
            // Em profundidades muito altas, seja ainda mais agressivo
            base_reduction + 1
        } else {
            base_reduction
        }
    }
    
    /// Verifica se movimento é tático (não deve ser reduzido)
    pub fn is_tactical_move(board: &Board, mv: Move) -> bool {
        let to_bb = 1u64 << mv.to;
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };
        
        // Capturas
        if (enemy_pieces & to_bb) != 0 {
            return true;
        }
        
        // Promoções
        if mv.promotion.is_some() {
            return true;
        }
        
        // Roque
        if mv.is_castling {
            return true;
        }
        
        // En passant
        if mv.is_en_passant {
            return true;
        }
        
        // Xeques (verificação rápida)
        let mut test_board = *board;
        if test_board.make_move(mv) {
            if test_board.is_king_in_check(!board.to_move) {
                return true;
            }
        }
        
        false
    }
    
    /// Verifica se movimento é "killer move" (movimentos que causaram cutoffs antes)
    pub fn is_killer_move(_mv: Move, _depth: u8) -> bool {
        // Implementação simples - expandir depois com killer move table
        false
    }
    
    /// Calcula redução ultra-agressiva para posições calmas
    pub fn get_ultra_reduction(move_index: usize, depth: u8, is_tactical: bool, in_check: bool) -> u8 {
        if is_tactical || in_check || depth < 3 {
            return 0;
        }
        
        // REDUÇÃO ULTRA-AGRESSIVA para depth 17
        match move_index {
            0..=1 => 0,     // Apenas primeiros 2 movimentos sem redução
            2..=3 => 1,     // Movimentos 3-4 - reduz 1
            4..=6 => 2,     // Movimentos 5-7 - reduz 2
            7..=9 => 3,     // Movimentos 8-10 - reduz 3
            10..=12 => 4,   // Movimentos 11-13 - reduz 4
            _ => depth.saturating_sub(2), // Movimentos tardios - redução máxima
        }
    }
}

/// Configurações de LMR ajustáveis
pub struct LMRConfig {
    pub enabled: bool,
    pub min_depth: u8,
    pub min_move_index: usize,
    pub max_reduction: u8,
    pub aggressive_mode: bool,
}

impl Default for LMRConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_depth: 3,
            min_move_index: 3,
            max_reduction: 4,
            aggressive_mode: true, // Modo agressivo para depth 17
        }
    }
}

impl LMRConfig {
    /// Configuração ultra-agressiva para profundidades altas
    pub fn ultra_aggressive() -> Self {
        Self {
            enabled: true,
            min_depth: 2,      // Reduz a partir de depth 2
            min_move_index: 2, // Reduz a partir do 3º movimento
            max_reduction: 6,  // Redução máxima de 6 níveis
            aggressive_mode: true,
        }
    }
    
    /// Calcula redução usando configuração
    pub fn calculate_reduction(&self, move_index: usize, depth: u8, is_tactical: bool) -> u8 {
        if !self.enabled || is_tactical || depth < self.min_depth || move_index < self.min_move_index {
            return 0;
        }
        
        if self.aggressive_mode {
            LateMovePruner::get_ultra_reduction(move_index, depth, is_tactical, false)
                .min(self.max_reduction)
        } else {
            LateMovePruner::get_reduction(move_index, depth, is_tactical, false)
                .min(self.max_reduction)
        }
    }
}