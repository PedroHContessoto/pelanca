use crate::core::*;
use crate::utils::*;

/// Análise tática ultra-rápida para detectar padrões táticos
pub struct TacticalAnalyzer;

impl TacticalAnalyzer {
    /// Detecta se posição tem potencial tático (xeques, capturas, ameaças)
    pub fn has_tactical_potential(board: &Board) -> bool {
        // Rei em xeque = alta prioridade tática
        if board.is_king_in_check(board.to_move) {
            return true;
        }
        
        // Peças desprotegidas = oportunidades táticas
        if Self::has_undefended_pieces(board) {
            return true;
        }
        
        // Rei exposto = possíveis ataques de mate
        if Self::is_king_exposed(board) {
            return true;
        }
        
        // Peças "penduradas" (atacadas por peças de menor valor)
        if Self::has_hanging_pieces(board) {
            return true;
        }
        
        false
    }
    
    /// Detecta peças desprotegidas (vulneráveis a capturas)
    fn has_undefended_pieces(board: &Board) -> bool {
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };
        
        // Verifica peças valiosas (não peões) desprotegidas
        let valuable_pieces = enemy_pieces & !(board.pawns | board.kings);
        let mut pieces = valuable_pieces;
        
        while pieces != 0 {
            let square = trailing_zeros(pieces) as u8;
            pieces = reset_lsb(pieces);
            
            // Se peça não está defendida por nenhuma peça amiga
            if !Self::is_square_defended(board, square, !board.to_move) {
                return true;
            }
        }
        
        false
    }
    
    /// Verifica se rei está exposto (poucas casas de fuga, peões fracos)
    fn is_king_exposed(board: &Board) -> bool {
        let enemy_color = !board.to_move;
        let enemy_king = if enemy_color == Color::White {
            board.kings & board.white_pieces
        } else {
            board.kings & board.black_pieces
        };
        
        if enemy_king == 0 { return false; }
        
        let king_square = trailing_zeros(enemy_king) as u8;
        let king_file = king_square % 8;
        let king_rank = king_square / 8;
        
        // Rei no centro = muito exposto
        if king_file >= 2 && king_file <= 5 && king_rank >= 2 && king_rank <= 5 {
            return true;
        }
        
        // Conta casas de fuga seguras
        let escape_squares = Self::count_king_escape_squares(board, king_square, enemy_color);
        escape_squares <= 2 // Rei com poucas fugas
    }
    
    /// Detecta peças "penduradas" (atacadas por peças de menor valor)
    fn has_hanging_pieces(board: &Board) -> bool {
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };
        
        // Verifica cavalos e bispos atacados por peões
        let minor_pieces = enemy_pieces & (board.knights | board.bishops);
        let my_pawns = if board.to_move == Color::White {
            board.white_pieces & board.pawns
        } else {
            board.black_pieces & board.pawns
        };
        
        let mut pieces = minor_pieces;
        while pieces != 0 {
            let square = trailing_zeros(pieces) as u8;
            pieces = reset_lsb(pieces);
            
            // Se peça menor está sendo atacada por peão
            if Self::is_attacked_by_pawns(board, square, board.to_move) {
                return true;
            }
        }
        
        // Verifica torres atacadas por peças menores
        let rooks = enemy_pieces & board.rooks;
        let mut rook_pieces = rooks;
        while rook_pieces != 0 {
            let square = trailing_zeros(rook_pieces) as u8;
            rook_pieces = reset_lsb(rook_pieces);
            
            if Self::is_attacked_by_minor_pieces(board, square, board.to_move) {
                return true;
            }
        }
        
        false
    }
    
    /// Conta casas de fuga seguras para o rei
    fn count_king_escape_squares(board: &Board, king_square: u8, king_color: Color) -> u32 {
        let king_attacks = crate::moves::king::get_king_attacks(king_square);
        let own_pieces = if king_color == Color::White {
            board.white_pieces
        } else {
            board.black_pieces
        };
        
        let possible_squares = king_attacks & !own_pieces;
        let mut safe_count = 0;
        let mut squares = possible_squares;
        
        while squares != 0 {
            let square = trailing_zeros(squares) as u8;
            squares = reset_lsb(squares);
            
            // Verifica se casa é segura (não atacada)
            if !board.is_square_attacked_by(square, !king_color) {
                safe_count += 1;
            }
        }
        
        safe_count
    }
    
    /// Verifica se casa está defendida por cor específica
    fn is_square_defended(board: &Board, square: u8, color: Color) -> bool {
        board.is_square_attacked_by(square, color)
    }
    
    /// Verifica se casa é atacada por peões
    fn is_attacked_by_pawns(board: &Board, square: u8, attacking_color: Color) -> bool {
        let pawn_attackers = crate::moves::pawn::get_pawn_attackers(square, attacking_color);
        let attacking_pawns = if attacking_color == Color::White {
            board.white_pieces & board.pawns
        } else {
            board.black_pieces & board.pawns
        };
        
        (pawn_attackers & attacking_pawns) != 0
    }
    
    /// Verifica se casa é atacada por peças menores (cavalos/bispos)
    fn is_attacked_by_minor_pieces(board: &Board, square: u8, attacking_color: Color) -> bool {
        let attacking_pieces = if attacking_color == Color::White {
            board.white_pieces
        } else {
            board.black_pieces
        };
        
        // Ataques de cavalos
        let knight_attacks = crate::moves::knight::get_knight_attacks(square);
        if (knight_attacks & board.knights & attacking_pieces) != 0 {
            return true;
        }
        
        // Ataques de bispos
        let all_pieces = board.white_pieces | board.black_pieces;
        let bishop_attacks = crate::moves::magic_bitboards::get_bishop_attacks_magic(square, all_pieces);
        if (bishop_attacks & board.bishops & attacking_pieces) != 0 {
            return true;
        }
        
        false
    }
}

/// Filtro adaptativo para movimentos não promissores
pub struct MoveFilter;

impl MoveFilter {
    /// Filtra movimentos claramente ruins, mas mantém diversidade
    pub fn filter_unpromising_moves(board: &Board, moves: &mut Vec<Move>) {
        let original_count = moves.len();
        
        // FILTRO AGRESSIVO para reduzir explosão combinatória
        // Mantém apenas os movimentos mais promissores
        
        let tactical_position = TacticalAnalyzer::has_tactical_potential(board);
        
        // FILTRO PRIMÁRIO: Remove movimentos claramente ruins
        moves.retain(|&mv| {
            Self::is_move_promising(board, mv, tactical_position)
        });
        
        // FILTRO SECUNDÁRIO: Se ainda há muitos movimentos, seja mais seletivo
        if moves.len() > 15 {
            moves.retain(|&mv| {
                Self::is_high_priority_move(board, mv, tactical_position)
            });
        }
        
        // FILTRO TERCIÁRIO: Limita drasticamente para depth alto
        let max_moves = if original_count > 25 { 6 } else { 8 }; // Máximo 6-8 movimentos
        
        if moves.len() > max_moves {
            // Ordena por qualidade e mantém apenas os melhores
            use crate::search::move_ordering::order_moves;
            order_moves(board, moves);
            moves.truncate(max_moves);
        }
        
        // GARANTIA MÍNIMA: Sempre mantém pelo menos 3 movimentos
        if moves.len() < 3 && original_count >= 3 {
            *moves = board.generate_legal_moves();
            
            // Filtra apenas os terríveis
            moves.retain(|&mv| {
                !Self::is_move_terrible(board, mv)
            });
            
            // Pega apenas os 5 melhores
            if moves.len() > 5 {
                use crate::search::move_ordering::order_moves;
                order_moves(board, moves);
                moves.truncate(5);
            }
        }
    }
    
    /// Verifica se movimento é de ALTA prioridade (filtro mais seletivo)
    fn is_high_priority_move(board: &Board, mv: Move, tactical_position: bool) -> bool {
        // SEMPRE mantém movimentos táticos críticos
        if Self::is_tactical_move(board, mv) {
            return true;
        }
        
        // SEMPRE mantém controle de centro
        if Self::is_center_move(mv) {
            return true;
        }
        
        // SEMPRE mantém movimentos de rei
        if Self::is_king_move(board, mv) {
            return true;
        }
        
        // Em posições táticas, mantém desenvolvimento
        if tactical_position && Self::is_development_move(board, mv) {
            return true;
        }
        
        false // Remove todo o resto para reduzir explosão combinatória
    }
    
    /// Verifica se movimento é promissor
    fn is_move_promising(board: &Board, mv: Move, tactical_position: bool) -> bool {
        // SEMPRE mantém movimentos táticos importantes
        if Self::is_tactical_move(board, mv) {
            return true;
        }
        
        // SEMPRE mantém desenvolvimento natural
        if Self::is_development_move(board, mv) {
            return true;
        }
        
        // SEMPRE mantém controle de centro
        if Self::is_center_move(mv) {
            return true;
        }
        
        // SEMPRE mantém movimentos de rei (roque, fuga)
        if Self::is_king_move(board, mv) {
            return true;
        }
        
        // Em posições táticas, filtra menos agressivamente
        if tactical_position {
            return !Self::is_obviously_bad(board, mv);
        }
        
        // Em posições calmas, pode filtrar mais
        !Self::is_unproductive_move(board, mv)
    }
    
    /// Verifica se movimento é terrível (filtragem ultra-conservadora)
    fn is_move_terrible(board: &Board, mv: Move) -> bool {
        // Nunca remove capturas, xeques, roques, promoções
        if Self::is_tactical_move(board, mv) {
            return false;
        }
        
        // Nunca remove movimentos de desenvolvimento
        if Self::is_development_move(board, mv) {
            return false;
        }
        
        // Remove apenas movimentos que claramente pioram a posição
        Self::is_clearly_blunder(board, mv)
    }
    
    /// Detecta movimentos táticos (alta prioridade)
    fn is_tactical_move(board: &Board, mv: Move) -> bool {
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
        
        // Xeques (testa rapidamente)
        let mut test_board = *board;
        if test_board.make_move(mv) {
            if test_board.is_king_in_check(!board.to_move) {
                return true;
            }
        }
        
        false
    }
    
    /// Detecta movimentos de desenvolvimento
    fn is_development_move(board: &Board, mv: Move) -> bool {
        let from_bb = 1u64 << mv.from;
        
        // Movimento de peças menores saindo da casa inicial
        if (board.knights & from_bb) != 0 || (board.bishops & from_bb) != 0 {
            const WHITE_BACK_RANK: u64 = 0x00000000000000FF;
            const BLACK_BACK_RANK: u64 = 0xFF00000000000000;
            
            let back_rank = if board.to_move == Color::White {
                WHITE_BACK_RANK
            } else {
                BLACK_BACK_RANK
            };
            
            // Saindo da casa inicial = desenvolvimento
            if (from_bb & back_rank) != 0 {
                return true;
            }
        }
        
        false
    }
    
    /// Detecta movimentos para o centro
    fn is_center_move(mv: Move) -> bool {
        const CENTER: u64 = 0x0000001818000000; // e4, e5, d4, d5
        const EXTENDED_CENTER: u64 = 0x00003C3C3C3C0000; // c3-f6
        
        let to_bb = 1u64 << mv.to;
        (to_bb & (CENTER | EXTENDED_CENTER)) != 0
    }
    
    /// Detecta movimentos de rei
    fn is_king_move(board: &Board, mv: Move) -> bool {
        let from_bb = 1u64 << mv.from;
        (board.kings & from_bb) != 0
    }
    
    /// Detecta movimentos obviamente ruins
    fn is_obviously_bad(board: &Board, mv: Move) -> bool {
        let from_bb = 1u64 << mv.from;
        let to_bb = 1u64 << mv.to;
        
        // Mover peça para casa atacada por peão inimigo (sem compensação)
        if !Self::is_tactical_move(board, mv) {
            if TacticalAnalyzer::is_attacked_by_pawns(board, mv.to, !board.to_move) {
                // Se não há compensação tática, pode ser ruim
                return true;
            }
        }
        
        false
    }
    
    /// Detecta movimentos improdutivos
    fn is_unproductive_move(board: &Board, mv: Move) -> bool {
        let from_bb = 1u64 << mv.from;
        
        // Mover a mesma peça repetidamente no início
        if board.halfmove_clock < 10 { // Primeiros 5 movimentos
            // Movimento de peça já desenvolvida (não urgente)
            if (board.knights & from_bb) != 0 || (board.bishops & from_bb) != 0 {
                const WHITE_BACK_RANK: u64 = 0x00000000000000FF;
                const BLACK_BACK_RANK: u64 = 0xFF00000000000000;
                
                let back_rank = if board.to_move == Color::White {
                    WHITE_BACK_RANK
                } else {
                    BLACK_BACK_RANK
                };
                
                // Se peça JÁ saiu da casa inicial, movimento pode ser menos urgente
                if (from_bb & back_rank) == 0 {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Detecta blunders claros
    fn is_clearly_blunder(board: &Board, mv: Move) -> bool {
        // Por agora, muito conservador - apenas casos extremos
        // Expandir conforme necessário
        false
    }
}