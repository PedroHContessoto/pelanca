// Move Ordering - Sistema inteligente de ordena��o de movimentos
// Cr�tico para efici�ncia do Alpha-Beta: bons movimentos primeiro = mais cortes

use crate::core::*;

/// Valores para ordenação de movimentos (maior = melhor)
const MVV_LVA_SCORES: [[i16; 6]; 6] = [
    // V�tima: P    N    B    R    Q    K
    [105, 205, 305, 405, 505, 605], // Atacante: Pawn
    [104, 204, 304, 404, 504, 604], // Atacante: Knight
    [103, 203, 303, 403, 503, 603], // Atacante: Bishop
    [102, 202, 302, 402, 502, 602], // Atacante: Rook
    [101, 201, 301, 401, 501, 601], // Atacante: Queen
    [100, 200, 300, 400, 500, 600], // Atacante: King
];

/// Bonus para movimentos especiais
const PROMOTION_BONUS: i16 = 800;
const CASTLE_BONUS: i16 = 50;
const EN_PASSANT_BONUS: i16 = 105; // MVV-LVA equivalente a PxP

/// Penalidades para movimentos ruins
const BAD_CAPTURE_PENALTY: i16 = -200;

/// Estrutura para armazenar hist�rico de movimentos
pub struct HistoryTable {
    /// Hist�ria por [cor][from][to]
    quiet_history: [[[i16; 64]; 64]; 2],
    /// Hist�ria de capturas por [piece][to][captured_piece]
    capture_history: [[[i16; 6]; 64]; 6],
    /// Butterfly boards para normaliza��o
    butterfly: [[[u32; 64]; 64]; 2],
}

impl HistoryTable {
    pub fn new() -> Self {
        HistoryTable {
            quiet_history: [[[0; 64]; 64]; 2],
            capture_history: [[[0; 6]; 64]; 6],
            butterfly: [[[0; 64]; 64]; 2],
        }
    }

    /// Atualiza hist�rico para movimento bom
    pub fn update_good_quiet(&mut self, color: Color, mv: Move, depth: u8) {
        let color_idx = if color == Color::White { 0 } else { 1 };
        let bonus = (depth as i16).pow(2).min(400);
        
        self.quiet_history[color_idx][mv.from as usize][mv.to as usize] += bonus;
        self.butterfly[color_idx][mv.from as usize][mv.to as usize] += 1;
        
        // Satura��o para evitar overflow
        if self.quiet_history[color_idx][mv.from as usize][mv.to as usize] > 16000 {
            self.age_history();
        }
    }

    /// Atualiza hist�rico para movimento ruim
    pub fn update_bad_quiet(&mut self, color: Color, mv: Move, depth: u8) {
        let color_idx = if color == Color::White { 0 } else { 1 };
        let penalty = (depth as i16).pow(2).min(400);
        
        self.quiet_history[color_idx][mv.from as usize][mv.to as usize] -= penalty;
        self.butterfly[color_idx][mv.from as usize][mv.to as usize] += 1;
    }

    /// Obt�m score do hist�rico
    pub fn get_quiet_score(&self, color: Color, mv: Move) -> i16 {
        let color_idx = if color == Color::White { 0 } else { 1 };
        self.quiet_history[color_idx][mv.from as usize][mv.to as usize]
    }

    /// Atualiza hist�rico de capturas
    pub fn update_capture_history(&mut self, board: &Board, mv: Move, good: bool, depth: u8) {
        if let Some(attacker_piece) = self.get_piece_at_square(board, mv.from) {
            if let Some(victim_piece) = self.get_captured_piece_kind(board, mv) {
                let attacker_idx = Self::piece_to_index(attacker_piece);
                let victim_idx = Self::piece_to_index(victim_piece);
                let delta = if good { (depth as i16).pow(2) } else { -(depth as i16).pow(2) };
                
                self.capture_history[attacker_idx][mv.to as usize][victim_idx] += delta;
            }
        }
    }

    /// Obt�m score do hist�rico de capturas
    pub fn get_capture_score(&self, board: &Board, mv: Move) -> i16 {
        if let Some(attacker_piece) = self.get_piece_at_square(board, mv.from) {
            if let Some(victim_piece) = self.get_captured_piece_kind(board, mv) {
                let attacker_idx = Self::piece_to_index(attacker_piece);
                let victim_idx = Self::piece_to_index(victim_piece);
                return self.capture_history[attacker_idx][mv.to as usize][victim_idx];
            }
        }
        0
    }

    /// Reduz valores de hist�rico para evitar overflow
    fn age_history(&mut self) {
        for color in 0..2 {
            for from in 0..64 {
                for to in 0..64 {
                    self.quiet_history[color][from][to] /= 2;
                }
            }
        }
        
        for piece in 0..6 {
            for to in 0..64 {
                for victim in 0..6 {
                    self.capture_history[piece][to][victim] /= 2;
                }
            }
        }
    }

    /// Limpa tabelas de hist�rico
    pub fn clear(&mut self) {
        self.quiet_history = [[[0; 64]; 64]; 2];
        self.capture_history = [[[0; 6]; 64]; 6];
        self.butterfly = [[[0; 64]; 64]; 2];
    }

    // Fun��es auxiliares

    fn get_piece_at_square(&self, board: &Board, square: u8) -> Option<PieceKind> {
        let bb = 1u64 << square;
        
        if (board.pawns & bb) != 0 { Some(PieceKind::Pawn) }
        else if (board.knights & bb) != 0 { Some(PieceKind::Knight) }
        else if (board.bishops & bb) != 0 { Some(PieceKind::Bishop) }
        else if (board.rooks & bb) != 0 { Some(PieceKind::Rook) }
        else if (board.queens & bb) != 0 { Some(PieceKind::Queen) }
        else if (board.kings & bb) != 0 { Some(PieceKind::King) }
        else { None }
    }

    fn get_captured_piece_kind(&self, board: &Board, mv: Move) -> Option<PieceKind> {
        if mv.is_en_passant {
            return Some(PieceKind::Pawn);
        }
        
        let to_bb = 1u64 << mv.to;
        let enemy_pieces = if board.to_move == Color::White { 
            board.black_pieces 
        } else { 
            board.white_pieces 
        };
        
        if (enemy_pieces & to_bb) == 0 {
            return None;
        }
        
        self.get_piece_at_square(board, mv.to)
    }

    fn piece_to_index(piece: PieceKind) -> usize {
        match piece {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5,
        }
    }
}

/// Sistema de ordena��o de movimentos
pub struct MoveOrderer {
    history: HistoryTable,
}

impl MoveOrderer {
    pub fn new() -> Self {
        MoveOrderer {
            history: HistoryTable::new(),
        }
    }

    /// Ordena lista de movimentos para m�xima efici�ncia Alpha-Beta
    pub fn order_moves(&self, board: &Board, moves: &mut Vec<Move>, tt_move: Option<Move>, ply: u16) {
        // Calcula scores para todos os movimentos
        let mut move_scores: Vec<(Move, i16)> = moves.iter()
            .map(|&mv| (mv, self.score_move(board, mv, tt_move, ply)))
            .collect();

        // Ordena por score (maior primeiro)
        move_scores.sort_by(|a, b| b.1.cmp(&a.1));

        // Atualiza a lista original
        *moves = move_scores.into_iter().map(|(mv, _)| mv).collect();
    }

    /// Calcula score de um movimento para ordena��o
    fn score_move(&self, board: &Board, mv: Move, tt_move: Option<Move>, _ply: u16) -> i16 {
        // 1. Movimento da TT tem prioridade m�xima
        if let Some(tt_mv) = tt_move {
            if mv.from == tt_mv.from && mv.to == tt_mv.to && mv.promotion == tt_mv.promotion {
                return 10000;
            }
        }

        let mut score = 0;

        // 2. Promo��es (especialmente rainha)
        if let Some(promotion) = mv.promotion {
            score += PROMOTION_BONUS;
            if promotion == PieceKind::Queen {
                score += 200;
            }
        }

        // 3. Capturas usando MVV-LVA
        if self.is_capture(board, mv) {
            score += self.mvv_lva_score(board, mv);
            
            // Bonus do hist�rico de capturas
            score += self.history.get_capture_score(board, mv) / 10;
            
            // Penaliza capturas ruins (SEE negativo)
            if self.is_bad_capture(board, mv) {
                score += BAD_CAPTURE_PENALTY;
            }
        } else {
            // 4. Movimentos silenciosos: hist�rico + heur�sticas
            score += self.history.get_quiet_score(board.to_move, mv) / 10;
            
            // Bonus para roque
            if mv.is_castling {
                score += CASTLE_BONUS;
            }
            
            // Heur�sticas posicionais b�sicas
            score += self.positional_score(board, mv);
        }

        // 5. En passant
        if mv.is_en_passant {
            score += EN_PASSANT_BONUS;
        }

        score
    }

    /// Verifica se movimento � captura
    fn is_capture(&self, board: &Board, mv: Move) -> bool {
        if mv.is_en_passant {
            return true;
        }
        
        let to_bb = 1u64 << mv.to;
        let enemy_pieces = if board.to_move == Color::White { 
            board.black_pieces 
        } else { 
            board.white_pieces 
        };
        
        (enemy_pieces & to_bb) != 0
    }

    /// Calcula score MVV-LVA (Most Valuable Victim - Least Valuable Attacker)
    fn mvv_lva_score(&self, board: &Board, mv: Move) -> i16 {
        let attacker = self.get_piece_at_square(board, mv.from);
        let victim = if mv.is_en_passant {
            Some(PieceKind::Pawn)
        } else {
            self.get_piece_at_square(board, mv.to)
        };

        match (attacker, victim) {
            (Some(att), Some(vic)) => {
                let att_idx = Self::piece_to_index(att);
                let vic_idx = Self::piece_to_index(vic);
                MVV_LVA_SCORES[att_idx][vic_idx]
            }
            _ => 0,
        }
    }

    /// Verifica se captura � ruim usando SEE aproximado
    fn is_bad_capture(&self, board: &Board, mv: Move) -> bool {
        // Implementa��o simplificada de SEE (Static Exchange Evaluation)
        // Em vers�o completa, calcularia todas as trocas poss�veis
        
        let attacker_value = self.get_piece_value(board, mv.from);
        let victim_value = if mv.is_en_passant {
            100 // Valor do pe�o
        } else {
            self.get_piece_value(board, mv.to)
        };
        
        // Se a v�tima vale menos que o atacante e est� defendida, pode ser ruim
        if victim_value < attacker_value {
            // Verifica se casa de destino est� defendida
            return self.is_square_defended(board, mv.to, !board.to_move);
        }
        
        false
    }

    /// Calcula score posicional b�sico para movimentos silenciosos
    fn positional_score(&self, board: &Board, mv: Move) -> i16 {
        let mut score = 0;
        
        // Movimento para centro
        let to_file = (mv.to % 8) as i16;
        let to_rank = (mv.to / 8) as i16;
        let center_distance = (3.5 - to_file as f32).abs() + (3.5 - to_rank as f32).abs();
        score += (10.0 - center_distance * 2.0) as i16;
        
        // Desenvolvimento de pe�as (cavalos e bispos para casa melhor)
        if let Some(piece) = self.get_piece_at_square(board, mv.from) {
            match piece {
                PieceKind::Knight | PieceKind::Bishop => {
                    // Bonus por sair da primeira fileira
                    let from_rank = mv.from / 8;
                    let to_rank = mv.to / 8;
                    
                    if board.to_move == Color::White {
                        if from_rank == 0 && to_rank > 0 {
                            score += 20;
                        }
                    } else {
                        if from_rank == 7 && to_rank < 7 {
                            score += 20;
                        }
                    }
                }
                _ => {}
            }
        }
        
        score
    }

    /// Atualiza hist�rico ap�s beta cutoff
    pub fn update_history_cutoff(&mut self, board: &Board, mv: Move, depth: u8, quiet_moves: &[Move]) {
        if self.is_capture(board, mv) {
            // Movimento que causou cutoff � bom
            self.history.update_capture_history(board, mv, true, depth);
        } else {
            // Movimento silencioso que causou cutoff
            self.history.update_good_quiet(board.to_move, mv, depth);
        }
        
        // Movimentos tentados antes do cutoff s�o ruins
        for &bad_move in quiet_moves {
            if bad_move.from != mv.from || bad_move.to != mv.to {
                if self.is_capture(board, bad_move) {
                    self.history.update_capture_history(board, bad_move, false, depth);
                } else {
                    self.history.update_bad_quiet(board.to_move, bad_move, depth);
                }
            }
        }
    }

    /// Limpa tabelas de hist�rico
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    // Fun��es auxiliares

    fn get_piece_at_square(&self, board: &Board, square: u8) -> Option<PieceKind> {
        let bb = 1u64 << square;
        
        if (board.pawns & bb) != 0 { Some(PieceKind::Pawn) }
        else if (board.knights & bb) != 0 { Some(PieceKind::Knight) }
        else if (board.bishops & bb) != 0 { Some(PieceKind::Bishop) }
        else if (board.rooks & bb) != 0 { Some(PieceKind::Rook) }
        else if (board.queens & bb) != 0 { Some(PieceKind::Queen) }
        else if (board.kings & bb) != 0 { Some(PieceKind::King) }
        else { None }
    }

    fn get_piece_value(&self, board: &Board, square: u8) -> i16 {
        if let Some(piece) = self.get_piece_at_square(board, square) {
            match piece {
                PieceKind::Pawn => 100,
                PieceKind::Knight => 320,
                PieceKind::Bishop => 330,
                PieceKind::Rook => 500,
                PieceKind::Queen => 900,
                PieceKind::King => 20000,
            }
        } else {
            0
        }
    }

    fn is_square_defended(&self, board: &Board, square: u8, by_color: Color) -> bool {
        // Implementa��o b�sica - verifica se h� pe�as da cor especificada atacando a casa
        board.is_square_attacked_by(square, by_color)
    }

    fn piece_to_index(piece: PieceKind) -> usize {
        match piece {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5,
        }
    }
}

impl Default for MoveOrderer {
    fn default() -> Self {
        Self::new()
    }
}