use crate::core::*;
use super::TranspositionTable;

/// Move Ordering - Ordena movimentos para maximizar cutoffs no alpha-beta
pub struct MoveOrderer {
    // Cache para MVV-LVA values
    mvv_lva: [[i32; 6]; 6],
}

impl MoveOrderer {
    pub fn new() -> Self {
        let mut orderer = Self {
            mvv_lva: [[0; 6]; 6],
        };
        orderer.init_mvv_lva();
        orderer
    }

    /// Inicializa tabela MVV-LVA (Most Valuable Victim - Least Valuable Attacker)
    fn init_mvv_lva(&mut self) {
        let piece_values = [100, 320, 330, 500, 900, 20000]; // P, N, B, R, Q, K
        
        for victim in 0..6 {
            for attacker in 0..6 {
                self.mvv_lva[victim][attacker] = piece_values[victim] - piece_values[attacker] / 10;
            }
        }
    }

    /// Ordena movimentos por prioridade (melhor primeiro)
    pub fn order_moves(&self, mut moves: Vec<Move>, board: &Board, 
                      tt: &TranspositionTable, ply: u8) -> Vec<Move> {
        
        // Movimento da TT tem prioridade máxima
        let tt_move = tt.get_best_move(board.zobrist_hash);
        
        moves.sort_unstable_by(|&a, &b| {
            let score_a = self.score_move(a, board, tt_move, ply);
            let score_b = self.score_move(b, board, tt_move, ply);
            score_b.cmp(&score_a) // Ordem decrescente
        });

        moves
    }

    /// Pontua um movimento para ordenação
    fn score_move(&self, mv: Move, board: &Board, tt_move: Option<Move>, ply: u8) -> i32 {
        // 1. TT Move (prioridade máxima)
        if Some(mv) == tt_move {
            return 1_000_000;
        }

        // 2. Capturas (MVV-LVA)
        if let Some(captured) = board.get_piece_at(mv.to) {
            if let Some(attacker) = board.get_piece_at(mv.from) {
                return 900_000 + self.mvv_lva_score(attacker.kind, captured.kind);
            }
        }

        // 3. Promoções
        if let Some(promotion) = mv.promotion {
            return 800_000 + promotion.value();
        }

        // 4. En passant
        if mv.is_en_passant {
            return 850_000; // Entre capturas e promoções
        }

        // 5. Castling (geralmente bom)
        if mv.is_castling {
            return 700_000;
        }

        // 6. Killer moves - precisaríamos de uma referência ao engine
        // Placeholder para agora, implementaremos interface melhor depois
        
        // 7. History heuristic - também precisa de referência ao engine
        
        // 8. Movimentos "quietos" - ordenação básica por PST
        self.quiet_move_score(mv, board)
    }

    /// Versão melhorada que recebe scores de killer/history
    pub fn score_move_with_heuristics(&self, mv: Move, board: &Board, tt_move: Option<Move>, 
                                     ply: u8, is_killer: bool, history_score: i32) -> i32 {
        // 1. Verificação de MATE - mais seletiva mas eficaz
        if self.gives_check(board, mv) {
            // Só verifica mate para xeques (mais rápido que antes)
            if let Some(_) = board.get_piece_at(mv.from) {
                let mut temp_board = *board;
                temp_board.make_move(mv);
                
                // Verifica se é mate checando movimentos mais eficientemente
                let moves = temp_board.generate_all_moves();
                let mut legal_count = 0;
                for &m in moves.iter().take(5) { // Reduzido de 10 para 5
                    if temp_board.is_legal_move(m) {
                        legal_count += 1;
                        break;
                    }
                }
                
                if legal_count == 0 {
                    return 10_000_000; // MATE TEM PRIORIDADE ABSOLUTA!
                }
            }
            return 950_000; // Xeques têm alta prioridade
        }

        // 2. TT Move (segunda prioridade)
        if Some(mv) == tt_move {
            return 1_000_000;
        }

        // 3. Xeques (alta prioridade)
        if self.gives_check(board, mv) {
            return 950_000;
        }

        // 4. Capturas (MVV-LVA)
        if let Some(captured) = board.get_piece_at(mv.to) {
            if let Some(attacker) = board.get_piece_at(mv.from) {
                return 900_000 + self.mvv_lva_score(attacker.kind, captured.kind);
            }
        }

        // 5. Promoções
        if let Some(promotion) = mv.promotion {
            return 800_000 + promotion.value();
        }

        // 6. En passant
        if mv.is_en_passant {
            return 850_000;
        }

        // 7. Castling
        if mv.is_castling {
            return 700_000;
        }

        // 8. Killer moves
        if is_killer {
            return 600_000;
        }

        // 9. Ataques táticos (NOVO)
        let tactical_score = self.evaluate_tactical_threats(board, mv);
        if tactical_score > 0 {
            return 550_000 + tactical_score;
        }

        // 10. History heuristic
        if history_score > 0 {
            return 500_000 + (history_score / 10).min(50_000);
        }

        // 11. Movimentos quietos
        self.quiet_move_score(mv, board)
    }

    fn mvv_lva_score(&self, attacker: PieceKind, victim: PieceKind) -> i32 {
        let attacker_idx = self.piece_to_index(attacker);
        let victim_idx = self.piece_to_index(victim);
        self.mvv_lva[victim_idx][attacker_idx]
    }

    fn piece_to_index(&self, piece: PieceKind) -> usize {
        match piece {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5,
        }
    }

    /// Pontua movimentos "quietos" (não-capturas)
    fn quiet_move_score(&self, mv: Move, _board: &Board) -> i32 {
        // Movimentos para o centro são melhores
        let to_center_distance = self.distance_to_center(mv.to);
        let from_center_distance = self.distance_to_center(mv.from);
        
        let center_bonus = (from_center_distance - to_center_distance) * 10;
        
        // Pequeno bonus aleatório para quebrar empates
        let random_factor = (mv.from as i32 + mv.to as i32) % 10;
        
        center_bonus + random_factor
    }

    /// Calcula distância Manhattan ao centro (e4/d4/e5/d5)
    fn distance_to_center(&self, square: u8) -> i32 {
        let file = (square % 8) as i32;
        let rank = (square / 8) as i32;
        
        let center_file = 3.5;
        let center_rank = 3.5;
        
        let file_dist = ((file as f32) - center_file).abs() as i32;
        let rank_dist = ((rank as f32) - center_rank).abs() as i32;
        
        file_dist + rank_dist
    }

    /// Ordena apenas capturas (para quiescence)
    pub fn order_captures(&self, mut captures: Vec<Move>, board: &Board) -> Vec<Move> {
        captures.sort_unstable_by(|&a, &b| {
            let score_a = self.capture_score(a, board);
            let score_b = self.capture_score(b, board);
            score_b.cmp(&score_a)
        });
        
        captures
    }

    fn capture_score(&self, mv: Move, board: &Board) -> i32 {
        if let Some(captured) = board.get_piece_at(mv.to) {
            if let Some(attacker) = board.get_piece_at(mv.from) {
                return self.mvv_lva_score(attacker.kind, captured.kind);
            }
        }
        0
    }

    /// Verifica se um movimento dá xeque
    fn gives_check(&self, board: &Board, mv: Move) -> bool {
        // Verifica se há peça na casa de origem antes de fazer o movimento
        if board.get_piece_at(mv.from).is_none() {
            return false;
        }
        
        // Implementação simplificada - faz o movimento e verifica
        let mut temp_board = *board;
        temp_board.make_move(mv);
        temp_board.is_king_in_check(!board.to_move)
    }

    /// Avalia ameaças táticas de um movimento
    fn evaluate_tactical_threats(&self, board: &Board, mv: Move) -> i32 {
        let mut score = 0;

        // Verifica se o movimento ataca peças valiosas
        if let Some(piece) = board.get_piece_at(mv.from) {
            score += self.evaluate_piece_attacks_after_move(board, mv, piece);
        }

        // Bonus para movimentos que descobrem ataques
        score += self.evaluate_discovered_attacks(board, mv);

        // Bonus para movimentos que criam pins
        score += self.evaluate_pin_creation(board, mv);

        score.min(40_000) // Limita o bonus tático
    }

    fn evaluate_piece_attacks_after_move(&self, board: &Board, mv: Move, piece: Piece) -> i32 {
        let mut attack_score = 0;

        match piece.kind {
            PieceKind::Queen => {
                // Dama pode atacar em múltiplas direções
                attack_score += self.count_queen_attacks_from_square(board, mv.to, piece.color) * 15;
            }
            PieceKind::Rook => {
                // Torre ataca em linhas e colunas
                attack_score += self.count_rook_attacks_from_square(board, mv.to, piece.color) * 10;
            }
            PieceKind::Bishop => {
                // Bispo ataca em diagonais
                attack_score += self.count_bishop_attacks_from_square(board, mv.to, piece.color) * 8;
            }
            PieceKind::Knight => {
                // Cavalo pode fazer forks
                attack_score += self.count_knight_attacks_from_square(board, mv.to, piece.color) * 12;
            }
            _ => {}
        }

        attack_score
    }

    fn count_queen_attacks_from_square(&self, board: &Board, square: u8, color: Color) -> i32 {
        // Combinação de torre + bispo
        self.count_rook_attacks_from_square(board, square, color) +
        self.count_bishop_attacks_from_square(board, square, color)
    }

    fn count_rook_attacks_from_square(&self, board: &Board, square: u8, color: Color) -> i32 {
        let mut attacks = 0;
        let enemy_pieces = if color == Color::White { board.black_pieces } else { board.white_pieces };

        // Simplificado: verifica se há peças inimigas na mesma linha/coluna
        let file_mask = 0x0101010101010101u64 << (square % 8);
        let rank_mask = 0xFFu64 << (square & 56);

        if (enemy_pieces & file_mask) != 0 {
            attacks += (enemy_pieces & file_mask).count_ones() as i32;
        }
        if (enemy_pieces & rank_mask) != 0 {
            attacks += (enemy_pieces & rank_mask).count_ones() as i32;
        }

        attacks
    }

    fn count_bishop_attacks_from_square(&self, board: &Board, square: u8, color: Color) -> i32 {
        let mut attacks = 0;
        let enemy_pieces = if color == Color::White { board.black_pieces } else { board.white_pieces };

        // Simplificado: conta peças inimigas em diagonais próximas
        let file = square % 8;
        let rank = square / 8;

        // Verifica diagonais principais (implementação básica)
        for delta in [-9, -7, 7, 9] {
            let target = square as i8 + delta;
            if target >= 0 && target < 64 {
                let target_file = (target % 8) as u8;
                let target_rank = (target / 8) as u8;
                
                // Verifica se o movimento diagonal é válido
                if (target_file as i8 - file as i8).abs() == (target_rank as i8 - rank as i8).abs() {
                    let target_bb = 1u64 << target;
                    if (enemy_pieces & target_bb) != 0 {
                        attacks += 1;
                    }
                }
            }
        }

        attacks
    }

    fn count_knight_attacks_from_square(&self, board: &Board, square: u8, color: Color) -> i32 {
        let enemy_pieces = if color == Color::White { board.black_pieces } else { board.white_pieces };
        
        // Usa a função de ataque de cavalo já implementada na evaluation
        let knight_attacks = self.generate_knight_attacks_simple(square);
        (knight_attacks & enemy_pieces).count_ones() as i32
    }

    fn generate_knight_attacks_simple(&self, square: u8) -> u64 {
        let sq = square as i8;
        let mut attacks = 0u64;
        
        let moves = [
            sq + 17, sq + 15, sq + 10, sq + 6,
            sq - 6, sq - 10, sq - 15, sq - 17
        ];
        
        for &mv in &moves {
            if mv >= 0 && mv < 64 {
                let file_diff = (mv % 8 - sq % 8).abs();
                let rank_diff = (mv / 8 - sq / 8).abs();
                
                if (file_diff == 2 && rank_diff == 1) || (file_diff == 1 && rank_diff == 2) {
                    attacks |= 1u64 << mv;
                }
            }
        }
        
        attacks
    }

    fn evaluate_discovered_attacks(&self, board: &Board, mv: Move) -> i32 {
        // Bonus simples por movimentos que podem descobrir ataques
        let from_file = mv.from % 8;
        let from_rank = mv.from / 8;
        let to_file = mv.to % 8;
        let to_rank = mv.to / 8;

        // Se movimento é em direção diferente (pode descobrir ataque)
        if from_file != to_file && from_rank != to_rank {
            return 15; // Bonus por possível discovered attack
        }

        0
    }

    fn evaluate_pin_creation(&self, board: &Board, mv: Move) -> i32 {
        // Bonus simples para movimentos que podem criar pins
        // Implementação básica: se move peça de longo alcance para linha/coluna/diagonal com múltiplas peças
        if let Some(piece) = board.get_piece_at(mv.from) {
            match piece.kind {
                PieceKind::Queen | PieceKind::Rook | PieceKind::Bishop => {
                    return 10; // Bonus por mover peça que pode criar pins
                }
                _ => {}
            }
        }

        0
    }
}

impl Default for MoveOrderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Utilitários para ordenação avançada (futuro)
pub struct OrderingUtils;

impl OrderingUtils {
    /// SEE (Static Exchange Evaluation) - avalia se uma captura é vantajosa
    pub fn see_capture(board: &Board, mv: Move) -> i32 {
        // Implementação placeholder
        // TODO: Implementar SEE completo
        if let Some(captured) = board.get_piece_at(mv.to) {
            captured.kind.value()
        } else {
            0
        }
    }
}