use crate::core::*;
use crate::engine::TranspositionTable;

/// Ordena movimentos para maximizar podas Alpha-Beta
pub fn order_moves(board: &Board, moves: &mut Vec<Move>, tt: Option<&mut TranspositionTable>) {
    // Busca melhor movimento da TT primeiro
    let tt_move = tt.and_then(|table| table.get_best_move(board.zobrist_hash));
    
    moves.sort_by_key(|&mv| {
        let mut score = 0;
        
        // 1. TT Move - máxima prioridade
        if Some(mv) == tt_move {
            score += 10000;
        }
        
        // 2. Capturas ordenadas por MVV-LVA (Most Valuable Victim - Least Valuable Attacker)
        if is_capture(board, mv) {
            let victim_value = get_piece_value_at_square(board, mv.to);
            let attacker_value = get_piece_value_at_square(board, mv.from);
            score += 1000 + victim_value - attacker_value / 10;
        }
        
        // 3. Promoções
        if mv.promotion.is_some() {
            score += 900;
        }
        
        // 4. Xeques
        if gives_check(board, mv) {
            score += 50;
        }
        
        // 5. Controle do centro
        score += center_control_bonus(mv);
        
        // 6. Desenvolvimento de peças
        score += development_bonus(board, mv);
        
        -score // Negativo para ordem decrescente
    });
}

/// Verifica se movimento é uma captura
fn is_capture(board: &Board, mv: Move) -> bool {
    let to_bb = 1u64 << mv.to;
    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };
    
    (enemy_pieces & to_bb) != 0 || mv.is_en_passant
}

/// Valor da peça em uma casa
fn get_piece_value_at_square(board: &Board, square: u8) -> i32 {
    let square_bb = 1u64 << square;
    
    if (board.pawns & square_bb) != 0 { 100 }
    else if (board.knights & square_bb) != 0 { 320 }
    else if (board.bishops & square_bb) != 0 { 330 }
    else if (board.rooks & square_bb) != 0 { 500 }
    else if (board.queens & square_bb) != 0 { 900 }
    else if (board.kings & square_bb) != 0 { 10000 }
    else { 0 }
}

/// Verifica se movimento dá xeque (simplificado)
fn gives_check(board: &Board, mv: Move) -> bool {
    // Implementação simplificada - poderia ser otimizada
    let mut test_board = *board;
    if test_board.make_move(mv) {
        let enemy_color = !board.to_move;
        test_board.is_king_in_check(enemy_color)
    } else {
        false
    }
}

/// Bônus por controle do centro
fn center_control_bonus(mv: Move) -> i32 {
    // Casas centrais: e4, e5, d4, d5
    match mv.to {
        27 | 28 | 35 | 36 => 30, // e4, d4, e5, d5
        18 | 19 | 20 | 21 | 26 | 29 | 34 | 37 | 42 | 43 | 44 | 45 => 10, // Centro expandido
        _ => 0
    }
}

/// Bônus por desenvolvimento de peças
fn development_bonus(board: &Board, mv: Move) -> i32 {
    let from_bb = 1u64 << mv.from;
    
    // Desenvolvimento de cavalos e bispos
    if (board.knights & from_bb) != 0 || (board.bishops & from_bb) != 0 {
        // Saindo da fileira inicial
        let initial_rank = if board.to_move == Color::White { 
            mv.from < 16 // Primeira e segunda fileiras
        } else { 
            mv.from > 47 // Sétima e oitava fileiras
        };
        
        if initial_rank {
            return 20;
        }
    }
    
    0
}

/// Killer moves heuristic (movimentos que causaram cutoffs)
pub struct KillerMoves {
    killers: [[Option<Move>; 2]; 64], // [depth][slot]
}

impl KillerMoves {
    pub fn new() -> Self {
        Self {
            killers: [[None; 2]; 64],
        }
    }
    
    /// Adiciona killer move
    pub fn add_killer(&mut self, depth: u8, mv: Move) {
        let depth = depth as usize;
        if depth < 64 {
            // Shift: move o primeiro para segundo, adiciona novo como primeiro
            self.killers[depth][1] = self.killers[depth][0];
            self.killers[depth][0] = Some(mv);
        }
    }
    
    /// Verifica se movimento é killer
    pub fn is_killer(&self, depth: u8, mv: Move) -> Option<usize> {
        let depth = depth as usize;
        if depth < 64 {
            if self.killers[depth][0] == Some(mv) {
                Some(0) // Primeiro killer
            } else if self.killers[depth][1] == Some(mv) {
                Some(1) // Segundo killer
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// History heuristic (movimentos que historicamente foram bons)
pub struct HistoryTable {
    history: [[[i32; 64]; 64]; 2], // [color][from][to]
}

impl HistoryTable {
    pub fn new() -> Self {
        Self {
            history: [[[0; 64]; 64]; 2],
        }
    }
    
    /// Atualiza history table
    pub fn update(&mut self, color: Color, mv: Move, depth: u8) {
        let color_idx = if color == Color::White { 0 } else { 1 };
        let bonus = (depth as i32) * (depth as i32);
        
        self.history[color_idx][mv.from as usize][mv.to as usize] += bonus;
        
        // Evita overflow
        if self.history[color_idx][mv.from as usize][mv.to as usize] > 10000 {
            self.age_history();
        }
    }
    
    /// Obtém score do history
    pub fn get_score(&self, color: Color, mv: Move) -> i32 {
        let color_idx = if color == Color::White { 0 } else { 1 };
        self.history[color_idx][mv.from as usize][mv.to as usize]
    }
    
    /// Reduz todos os valores (aging)
    fn age_history(&mut self) {
        for color in 0..2 {
            for from in 0..64 {
                for to in 0..64 {
                    self.history[color][from][to] /= 2;
                }
            }
        }
    }
}

/// Ordenação completa com killer moves e history
pub fn order_moves_advanced(
    board: &Board, 
    moves: &mut Vec<Move>, 
    depth: u8,
    tt: Option<&mut TranspositionTable>,
    killers: &KillerMoves,
    history: &HistoryTable
) {
    let tt_move = tt.and_then(|table| table.get_best_move(board.zobrist_hash));
    
    moves.sort_by_key(|&mv| {
        let mut score = 0;
        
        // 1. TT Move
        if Some(mv) == tt_move {
            score += 100000;
        }
        
        // 2. Capturas (MVV-LVA)
        else if is_capture(board, mv) {
            let victim_value = get_piece_value_at_square(board, mv.to);
            let attacker_value = get_piece_value_at_square(board, mv.from);
            score += 10000 + victim_value - attacker_value / 10;
        }
        
        // 3. Killer moves
        else if let Some(killer_slot) = killers.is_killer(depth, mv) {
            score += 1000 - killer_slot as i32 * 100; // Primeiro killer > segundo killer
        }
        
        // 4. History heuristic
        else {
            score += history.get_score(board.to_move, mv);
        }
        
        // 5. Bônus adicionais
        if mv.promotion.is_some() {
            score += 5000;
        }
        
        if gives_check(board, mv) {
            score += 50;
        }
        
        score += center_control_bonus(mv);
        score += development_bonus(board, mv);
        
        -score
    });
}