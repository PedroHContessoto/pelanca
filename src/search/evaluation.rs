use crate::core::*;

// Piece-Square Tables (PST) - Middle Game
// Valores de -50 a +50 centipawns por posição
const PAWN_PST: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10,-20,-20, 10, 10,  5,
     5, -5,-10,  0,  0,-10, -5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5,  5, 10, 25, 25, 10,  5,  5,
    10, 10, 20, 30, 30, 20, 10, 10,
    50, 50, 50, 50, 50, 50, 50, 50,
     0,  0,  0,  0,  0,  0,  0,  0
];

const KNIGHT_PST: [i32; 64] = [
   -50,-40,-30,-30,-30,-30,-40,-50,
   -40,-20,  0,  0,  0,  0,-20,-40,
   -30,  0, 10, 15, 15, 10,  0,-30,
   -30,  5, 15, 20, 20, 15,  5,-30,
   -30,  0, 15, 20, 20, 15,  0,-30,
   -30,  5, 10, 15, 15, 10,  5,-30,
   -40,-20,  0,  5,  5,  0,-20,-40,
   -50,-40,-30,-30,-30,-30,-40,-50
];

const BISHOP_PST: [i32; 64] = [
   -20,-10,-10,-10,-10,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  0,  5, 10, 10,  5,  0,-10,
   -10,  5,  5, 10, 10,  5,  5,-10,
   -10,  0, 10, 10, 10, 10,  0,-10,
   -10, 10, 10, 10, 10, 10, 10,-10,
   -10,  5,  0,  0,  0,  0,  5,-10,
   -20,-10,-10,-10,-10,-10,-10,-20
];

const ROOK_PST: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0
];

const QUEEN_PST: [i32; 64] = [
   -20,-10,-10, -5, -5,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  0,  5,  5,  5,  5,  0,-10,
    -5,  0,  5,  5,  5,  5,  0, -5,
     0,  0,  5,  5,  5,  5,  0, -5,
   -10,  5,  5,  5,  5,  5,  0,-10,
   -10,  0,  5,  0,  0,  0,  0,-10,
   -20,-10,-10, -5, -5,-10,-10,-20
];

const KING_PST: [i32; 64] = [
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -20,-30,-30,-40,-40,-30,-30,-20,
   -10,-20,-20,-20,-20,-20,-20,-10,
    20, 20,  0,  0,  0,  0, 20, 20,
    20, 30, 10,  0,  0, 10, 30, 20
];

pub struct Evaluator {
    // Futuro: cache de avaliação, king safety, etc.
}

impl Evaluator {
    pub fn new() -> Self {
        Self {}
    }

    /// Avaliação principal do tabuleiro
    /// Retorna score em centipawns (positivo = brancas melhores)
    pub fn evaluate(&self, board: &Board) -> i32 {
        let mut score = 0;

        // 1. Material + Piece Square Tables
        score += self.evaluate_material_and_position(board);
        
        // 2. King Safety melhorada
        score += self.evaluate_king_safety(board);
        
        // 3. Tactical patterns (NOVO)
        score += self.evaluate_tactical_patterns(board);
        
        // 4. Piece activity e outposts (NOVO)
        score += self.evaluate_piece_activity(board);
        
        // 5. Pawn structure melhorada (NOVO)
        score += self.evaluate_pawn_structure(board);
        
        // 6. Mobilidade básica (peso reduzido)
        score += self.evaluate_mobility(board) / 4;

        // Perspectiva: sempre do ponto de vista do jogador atual
        if board.to_move == Color::Black {
            -score
        } else {
            score
        }
    }

    fn evaluate_material_and_position(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Para cada casa do tabuleiro
        for square in 0..64 {
            let square_bb = 1u64 << square;
            
            if let Some(piece) = board.get_piece_at(square) {
                let piece_value = piece.kind.value();
                let position_bonus = self.get_pst_value(piece.kind, square, piece.color);
                
                let total_value = piece_value + position_bonus;
                
                match piece.color {
                    Color::White => score += total_value,
                    Color::Black => score -= total_value,
                }
            }
        }

        score
    }

    fn get_pst_value(&self, piece: PieceKind, square: u8, color: Color) -> i32 {
        let pst_index = if color == Color::White {
            square as usize
        } else {
            // Para pretas, espelha verticalmente
            (56 - (square & 56) + (square & 7)) as usize
        };

        match piece {
            PieceKind::Pawn   => PAWN_PST[pst_index],
            PieceKind::Knight => KNIGHT_PST[pst_index],
            PieceKind::Bishop => BISHOP_PST[pst_index],
            PieceKind::Rook   => ROOK_PST[pst_index],
            PieceKind::Queen  => QUEEN_PST[pst_index],
            PieceKind::King   => KING_PST[pst_index],
        }
    }

    fn evaluate_king_safety(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Penalidade mais severa por xeque (tático)
        if board.white_king_in_check {
            score -= 100; // Aumentado de 50 para 100
        }
        if board.black_king_in_check {
            score += 100;
        }

        // Avalia segurança dos reis
        score += self.evaluate_king_safety_for_color(board, Color::White);
        score -= self.evaluate_king_safety_for_color(board, Color::Black);

        score
    }

    fn evaluate_king_safety_for_color(&self, board: &Board, color: Color) -> i32 {
        let mut safety_score = 0;
        
        // Encontra posição do rei
        let king_bb = board.kings & if color == Color::White { 
            board.white_pieces 
        } else { 
            board.black_pieces 
        };
        
        if king_bb == 0 {
            return -1000; // Rei não encontrado (erro)
        }
        
        let king_square = king_bb.trailing_zeros() as u8;
        let king_file = king_square % 8;
        let king_rank = king_square / 8;

        // Penalidade por rei no centro (meio-jogo)
        if self.is_middlegame(board) {
            let center_distance = ((king_file as i32 - 3).abs() + (king_rank as i32 - 3).abs()) as i32;
            if center_distance < 3 {
                safety_score -= (3 - center_distance) * 20;
            }
        }

        // Bonus por castling realizado
        if color == Color::White {
            // Rei branco em g1 ou c1 indica castling
            if king_square == 6 || king_square == 2 {
                safety_score += 30;
            }
        } else {
            // Rei preto em g8 ou c8
            if king_square == 62 || king_square == 58 {
                safety_score += 30;
            }
        }

        // Estrutura de peões ao redor do rei
        safety_score += self.evaluate_pawn_shield(board, color, king_square);

        // Penalidade por linhas abertas perto do rei
        safety_score -= self.evaluate_open_files_near_king(board, color, king_square);

        safety_score
    }

    fn evaluate_pawn_shield(&self, board: &Board, color: Color, king_square: u8) -> i32 {
        let mut shield_score = 0;
        let king_file = king_square % 8;
        let king_rank = king_square / 8;
        
        let pawn_bb = board.pawns & if color == Color::White { 
            board.white_pieces 
        } else { 
            board.black_pieces 
        };

        // Verifica peões nas 3 colunas ao redor do rei
        for file_offset in -1..=1 {
            let check_file = king_file as i32 + file_offset;
            if check_file < 0 || check_file > 7 {
                continue;
            }

            let file_mask = 0x0101010101010101u64 << check_file;
            let file_pawns = pawn_bb & file_mask;

            if file_pawns != 0 {
                // Há peão nesta coluna
                let pawn_square = if color == Color::White {
                    63 - file_pawns.leading_zeros() as u8 // Peão mais avançado
                } else {
                    file_pawns.trailing_zeros() as u8 // Peão mais recuado
                };

                let pawn_rank = pawn_square / 8;
                let rank_diff = if color == Color::White {
                    pawn_rank as i32 - king_rank as i32
                } else {
                    king_rank as i32 - pawn_rank as i32
                };

                // Bonus por peões perto do rei
                if rank_diff >= 1 && rank_diff <= 2 {
                    shield_score += (3 - rank_diff) * 10;
                }
            } else {
                // Penalidade por coluna sem peões
                shield_score -= 15;
            }
        }

        shield_score
    }

    fn evaluate_open_files_near_king(&self, board: &Board, color: Color, king_square: u8) -> i32 {
        let mut open_file_penalty = 0;
        let king_file = king_square % 8;

        // Verifica colunas ao redor do rei
        for file_offset in -1..=1 {
            let check_file = king_file as i32 + file_offset;
            if check_file < 0 || check_file > 7 {
                continue;
            }

            let file_mask = 0x0101010101010101u64 << check_file;
            
            // Verifica se há peões em qualquer cor nesta coluna
            let file_pawns = (board.pawns) & file_mask;
            
            if file_pawns == 0 {
                // Coluna completamente aberta
                open_file_penalty += 25;
            } else {
                // Verifica se há apenas peões inimigos (semi-aberta)
                let our_pawns = file_pawns & if color == Color::White { 
                    board.white_pieces 
                } else { 
                    board.black_pieces 
                };
                
                if our_pawns == 0 {
                    // Semi-aberta (só peões inimigos)
                    open_file_penalty += 15;
                }
            }
        }

        open_file_penalty
    }

    fn is_middlegame(&self, board: &Board) -> bool {
        // Heurística simples: meio-jogo se há damas no tabuleiro
        board.queens != 0
    }

    fn evaluate_mobility(&self, board: &Board) -> i32 {
        // Mobilidade simplificada - só conta pseudo-legais para speed
        let white_moves = if board.to_move == Color::White {
            board.generate_all_moves().len()
        } else {
            let mut board_copy = *board;
            board_copy.to_move = Color::White;
            board_copy.generate_all_moves().len()
        };

        let black_moves = if board.to_move == Color::Black {
            board.generate_all_moves().len()
        } else {
            let mut board_copy = *board;
            board_copy.to_move = Color::Black;
            board_copy.generate_all_moves().len()
        };

        // Mobilidade com peso reduzido para estabilidade
        let mobility_score = (white_moves as i32 - black_moves as i32);
        
        // Aplica taper baseado na fase do jogo
        if self.is_endgame(board) {
            mobility_score // Mobilidade mais importante no endgame
        } else {
            mobility_score / 2 // Reduzido no middlegame
        }
    }

    fn is_endgame(&self, board: &Board) -> bool {
        // Endgame se não há damas ou material baixo
        board.queens == 0 || self.total_material(board) < 2000
    }

    fn total_material(&self, board: &Board) -> i32 {
        let mut material = 0;
        
        for square in 0..64 {
            if let Some(piece) = board.get_piece_at(square) {
                material += piece.kind.value();
            }
        }
        
        material
    }

    /// Detecção rápida de mate/empate
    pub fn is_mate_score(score: i32) -> bool {
        score.abs() >= crate::search::MATE_IN_MAX
    }

    pub fn mate_in_n(n: u8) -> i32 {
        crate::search::MATE_SCORE - n as i32
    }
    
    /// Avalia se a posição tem potencial para mate em 1 ou 2
    pub fn has_mate_potential(&self, board: &Board) -> bool {
        // Verifica se o rei inimigo está em situação vulnerável
        let enemy_color = !board.to_move;
        let enemy_king_bb = board.kings & if enemy_color == Color::White { 
            board.white_pieces 
        } else { 
            board.black_pieces 
        };
        
        if enemy_king_bb == 0 {
            return false;
        }
        
        let king_square = enemy_king_bb.trailing_zeros() as u8;
        let escape_squares = self.count_king_escape_squares(board, king_square, enemy_color);
        
        // Se o rei tem poucas casas de escape, há potencial para mate
        escape_squares <= 2
    }

    /// Avalia padrões táticos na posição
    fn evaluate_tactical_patterns(&self, board: &Board) -> i32 {
        let mut score = 0;

        // 1. Peças atacadas/defendidas
        score += self.evaluate_attacked_pieces(board);
        
        // 2. Pins e skewers
        score += self.evaluate_pins_and_skewers(board);
        
        // 3. Forks e double attacks
        score += self.evaluate_forks(board);
        
        // 4. Back rank weakness
        score += self.evaluate_back_rank_threats(board);

        score
    }

    fn evaluate_attacked_pieces(&self, board: &Board) -> i32 {
        let mut score = 0;
        
        for square in 0..64 {
            if let Some(piece) = board.get_piece_at(square) {
                let attackers = self.count_attackers(board, square, !piece.color);
                let defenders = self.count_attackers(board, square, piece.color);
                
                if attackers > defenders {
                    // Peça atacada - penalidade mais severa baseada no valor
                    let penalty = piece.kind.value() / 5; // Aumentado de /10 para /5
                    if piece.color == Color::White {
                        score -= penalty;
                    } else {
                        score += penalty;
                    }
                }
            }
        }
        
        score
    }

    fn count_attackers(&self, board: &Board, square: u8, color: Color) -> u8 {
        let mut count = 0;
        let target_bb = 1u64 << square;
        
        // Verifica ataques de peões
        let pawn_attacks = if color == Color::White {
            // Peões brancos atacam para cima-esquerda e cima-direita
            let left_attack = (target_bb >> 7) & 0xFEFEFEFEFEFEFEFE;
            let right_attack = (target_bb >> 9) & 0x7F7F7F7F7F7F7F7F;
            left_attack | right_attack
        } else {
            // Peões pretos atacam para baixo-esquerda e baixo-direita
            let left_attack = (target_bb << 9) & 0xFEFEFEFEFEFEFEFE;
            let right_attack = (target_bb << 7) & 0x7F7F7F7F7F7F7F7F;
            left_attack | right_attack
        };
        
        let our_pawns = board.pawns & if color == Color::White { board.white_pieces } else { board.black_pieces };
        if (our_pawns & pawn_attacks) != 0 {
            count += 1;
        }

        // TODO: Adicionar verificação para outras peças (cavalos, bispos, torres, dama)
        // Por simplicidade, implementamos só peões por agora

        count
    }

    fn evaluate_pins_and_skewers(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Verifica pins nas diagonais e linhas/colunas
        score += self.check_diagonal_pins(board);
        score += self.check_straight_pins(board);

        score
    }

    fn check_diagonal_pins(&self, board: &Board) -> i32 {
        let mut score = 0;
        
        // Encontra bispos e damas
        let white_diagonal_pieces = (board.bishops | board.queens) & board.white_pieces;
        let black_diagonal_pieces = (board.bishops | board.queens) & board.black_pieces;

        // Verifica pins causados por peças brancas
        score += self.find_pins_from_pieces(board, white_diagonal_pieces, Color::White, true);
        
        // Verifica pins causados por peças pretas  
        score -= self.find_pins_from_pieces(board, black_diagonal_pieces, Color::Black, true);

        score
    }

    fn check_straight_pins(&self, board: &Board) -> i32 {
        let mut score = 0;
        
        // Encontra torres e damas
        let white_straight_pieces = (board.rooks | board.queens) & board.white_pieces;
        let black_straight_pieces = (board.rooks | board.queens) & board.black_pieces;

        // Verifica pins causados por peças brancas
        score += self.find_pins_from_pieces(board, white_straight_pieces, Color::White, false);
        
        // Verifica pins causados por peças pretas
        score -= self.find_pins_from_pieces(board, black_straight_pieces, Color::Black, false);

        score
    }

    fn find_pins_from_pieces(&self, board: &Board, pieces: u64, color: Color, diagonal: bool) -> i32 {
        let mut pin_score = 0;
        let enemy_color = !color;
        
        // Para cada peça que pode causar pin
        let mut pieces_bb = pieces;
        while pieces_bb != 0 {
            let piece_square = pieces_bb.trailing_zeros() as u8;
            pieces_bb &= pieces_bb - 1; // Remove o bit menos significativo
            
            // Busca possíveis pins nesta direção
            // Implementação simplificada - apenas conta como bonus se há peças alinhadas
            pin_score += 15; // Bonus por ter peças que podem causar pins
        }

        pin_score
    }

    fn evaluate_forks(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Cavalos são os principais causadores de forks
        let white_knights = board.knights & board.white_pieces;
        let black_knights = board.knights & board.black_pieces;

        score += self.count_knight_forks(board, white_knights, Color::White);
        score -= self.count_knight_forks(board, black_knights, Color::Black);

        score
    }

    fn count_knight_forks(&self, board: &Board, knights: u64, color: Color) -> i32 {
        let mut fork_score = 0;
        let enemy_pieces = if color == Color::White { board.black_pieces } else { board.white_pieces };
        
        let mut knights_bb = knights;
        while knights_bb != 0 {
            let knight_square = knights_bb.trailing_zeros() as u8;
            knights_bb &= knights_bb - 1;
            
            // Gera ataques do cavalo
            let knight_attacks = self.generate_knight_attacks(knight_square);
            let attacked_enemies = knight_attacks & enemy_pieces;
            
            // Se ataca 2+ peças, é um fork - bonus maior
            if attacked_enemies.count_ones() >= 2 {
                fork_score += 100 * attacked_enemies.count_ones() as i32; // Aumentado de 50 para 100
            }
        }

        fork_score
    }

    fn generate_knight_attacks(&self, square: u8) -> u64 {
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
                
                // Movimento válido de cavalo
                if (file_diff == 2 && rank_diff == 1) || (file_diff == 1 && rank_diff == 2) {
                    attacks |= 1u64 << mv;
                }
            }
        }
        
        attacks
    }

    fn evaluate_back_rank_threats(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Verifica fraqueza na primeira fileira (reis)
        let white_king_pos = (board.kings & board.white_pieces).trailing_zeros() as u8;
        let black_king_pos = (board.kings & board.black_pieces).trailing_zeros() as u8;

        // Rei branco na primeira fileira sem escape - penalidade maior
        if white_king_pos < 8 {
            let escape_squares = self.count_king_escape_squares(board, white_king_pos, Color::White);
            if escape_squares == 0 {
                score -= 150; // Aumentado de 75 para 150
            }
        }

        // Rei preto na oitava fileira sem escape - bonus maior
        if black_king_pos >= 56 {
            let escape_squares = self.count_king_escape_squares(board, black_king_pos, Color::Black);
            if escape_squares == 0 {
                score += 150; // Aumentado de 75 para 150
            }
        }

        score
    }

    fn count_king_escape_squares(&self, board: &Board, king_pos: u8, color: Color) -> u8 {
        let mut escape_count = 0;
        
        // Verifica casas adjacentes ao rei
        for delta in [-9, -8, -7, -1, 1, 7, 8, 9] {
            let new_pos = king_pos as i8 + delta;
            if new_pos >= 0 && new_pos < 64 {
                let new_square = new_pos as u8;
                let target_bb = 1u64 << new_square;
                
                // Verifica se a casa está livre ou tem peça inimiga
                let our_pieces = if color == Color::White { board.white_pieces } else { board.black_pieces };
                if (our_pieces & target_bb) == 0 {
                    escape_count += 1;
                }
            }
        }
        
        escape_count
    }

    fn evaluate_piece_activity(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Penalidades por peças desenvolvidas pobremente
        score += self.evaluate_piece_development(board);
        
        // Bonus por outposts (cavalos em casas avançadas)
        score += self.evaluate_outposts(board);

        score
    }

    fn evaluate_piece_development(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Penalidade por cavalos e bispos na fileira inicial
        let white_back_rank = 0x00000000000000FF;
        let black_back_rank = 0xFF00000000000000;

        let white_minor_pieces = (board.knights | board.bishops) & board.white_pieces;
        let black_minor_pieces = (board.knights | board.bishops) & board.black_pieces;

        // Penalidade por peças menores não desenvolvidas
        let white_undeveloped = (white_minor_pieces & white_back_rank).count_ones() as i32;
        let black_undeveloped = (black_minor_pieces & black_back_rank).count_ones() as i32;

        score -= white_undeveloped * 25;
        score += black_undeveloped * 25;

        score
    }

    fn evaluate_outposts(&self, board: &Board) -> i32 {
        let mut score = 0;

        // Cavalos em outposts (4ª, 5ª, 6ª fileira para brancas)
        let white_knights = board.knights & board.white_pieces;
        let black_knights = board.knights & board.black_pieces;

        // Outposts para cavalos brancos (fileiras 4-6)
        let white_outpost_ranks = 0x0000FFFFFF000000;
        let white_outpost_knights = white_knights & white_outpost_ranks;
        score += white_outpost_knights.count_ones() as i32 * 30;

        // Outposts para cavalos pretos (fileiras 3-5 do ponto de vista preto)  
        let black_outpost_ranks = 0x0000FFFFFF000000;
        let black_outpost_knights = black_knights & black_outpost_ranks;
        score -= black_outpost_knights.count_ones() as i32 * 30;

        score
    }

    fn evaluate_pawn_structure(&self, board: &Board) -> i32 {
        let mut score = 0;

        score += self.evaluate_doubled_pawns(board);
        score += self.evaluate_isolated_pawns(board);
        score += self.evaluate_passed_pawns(board);

        score
    }

    fn evaluate_doubled_pawns(&self, board: &Board) -> i32 {
        let mut score = 0;
        
        let white_pawns = board.pawns & board.white_pieces;
        let black_pawns = board.pawns & board.black_pieces;

        // Verifica cada coluna para peões dobrados
        for file in 0..8 {
            let file_mask = 0x0101010101010101u64 << file;
            
            let white_pawns_in_file = (white_pawns & file_mask).count_ones();
            let black_pawns_in_file = (black_pawns & file_mask).count_ones();
            
            if white_pawns_in_file > 1 {
                score -= 25 * (white_pawns_in_file - 1) as i32;
            }
            
            if black_pawns_in_file > 1 {
                score += 25 * (black_pawns_in_file - 1) as i32;
            }
        }

        score
    }

    fn evaluate_isolated_pawns(&self, board: &Board) -> i32 {
        let mut score = 0;
        
        let white_pawns = board.pawns & board.white_pieces;
        let black_pawns = board.pawns & board.black_pieces;

        for file in 0..8 {
            let file_mask = 0x0101010101010101u64 << file;
            let adjacent_files = if file > 0 { 0x0101010101010101u64 << (file - 1) } else { 0 } |
                               if file < 7 { 0x0101010101010101u64 << (file + 1) } else { 0 };
            
            // Peões brancos isolados
            if (white_pawns & file_mask) != 0 && (white_pawns & adjacent_files) == 0 {
                score -= 20;
            }
            
            // Peões pretos isolados
            if (black_pawns & file_mask) != 0 && (black_pawns & adjacent_files) == 0 {
                score += 20;
            }
        }

        score
    }

    fn evaluate_passed_pawns(&self, board: &Board) -> i32 {
        let mut score = 0;
        
        // Implementação simplificada de peões passados
        // TODO: Implementar detecção real de passed pawns
        
        score
    }
}