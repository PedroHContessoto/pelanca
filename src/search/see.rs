use crate::core::*;

/// Static Exchange Evaluation - avalia sequências de capturas
pub struct SEE;

impl SEE {
    /// Avalia uma captura usando Static Exchange Evaluation
    pub fn evaluate_capture(board: &Board, mv: Move) -> i32 {
        if !Self::is_capture_move(board, mv) {
            return 0;
        }
        
        // Simula a sequência de capturas
        let mut board_copy = *board;
        let mut gain = Vec::new();
        
        // Valor inicial da peça capturada
        let initial_victim = if let Some(piece) = board.get_piece_at(mv.to) {
            Self::get_piece_value(piece.kind)
        } else if mv.is_en_passant {
            100 // Peão en passant
        } else {
            return 0;
        };
        
        gain.push(initial_victim);
        
        // Faz o movimento inicial
        if !board_copy.make_move(mv) {
            return 0; // Movimento inválido
        }
        
        let mut current_square = mv.to;
        let mut attacking_color = !board.to_move;
        
        // Simula sequência de recapturas
        for depth in 1..=10 { // Limita profundidade para evitar loops
            if let Some(next_attacker) = Self::find_least_valuable_attacker(&board_copy, current_square, attacking_color) {
                // Valor da peça que será capturada (a que acabou de capturar)
                let victim_value = if let Some(piece) = board_copy.get_piece_at(current_square) {
                    Self::get_piece_value(piece.kind)
                } else {
                    break;
                };
                
                gain.push(victim_value);
                
                // Faz a recaptura
                let recapture_move = Move {
                    from: next_attacker,
                    to: current_square,
                    promotion: None,
                    is_castling: false,
                    is_en_passant: false,
                };
                
                if !board_copy.make_move(recapture_move) {
                    break;
                }
                
                attacking_color = !attacking_color;
            } else {
                break; // Não há mais atacantes
            }
        }
        
        // Calcula o resultado usando minimax
        Self::minimax_see(&gain)
    }
    
    /// Encontra o atacante de menor valor para uma casa
    fn find_least_valuable_attacker(board: &Board, square: u8, color: Color) -> Option<u8> {
        let attacking_pieces = if color == Color::White { 
            board.white_pieces 
        } else { 
            board.black_pieces 
        };
        
        // Procura atacantes em ordem de valor crescente
        
        // 1. Peões (valor 100)
        if let Some(attacker) = Self::find_piece_attacker(board, square, color, PieceKind::Pawn, attacking_pieces) {
            return Some(attacker);
        }
        
        // 2. Cavalos (valor 320)
        if let Some(attacker) = Self::find_piece_attacker(board, square, color, PieceKind::Knight, attacking_pieces) {
            return Some(attacker);
        }
        
        // 3. Bispos (valor 330)
        if let Some(attacker) = Self::find_piece_attacker(board, square, color, PieceKind::Bishop, attacking_pieces) {
            return Some(attacker);
        }
        
        // 4. Torres (valor 500)
        if let Some(attacker) = Self::find_piece_attacker(board, square, color, PieceKind::Rook, attacking_pieces) {
            return Some(attacker);
        }
        
        // 5. Dama (valor 900)
        if let Some(attacker) = Self::find_piece_attacker(board, square, color, PieceKind::Queen, attacking_pieces) {
            return Some(attacker);
        }
        
        // 6. Rei (último recurso)
        if let Some(attacker) = Self::find_piece_attacker(board, square, color, PieceKind::King, attacking_pieces) {
            return Some(attacker);
        }
        
        None
    }
    
    /// Encontra um atacante específico de um tipo de peça
    fn find_piece_attacker(board: &Board, square: u8, color: Color, piece_kind: PieceKind, attacking_pieces: u64) -> Option<u8> {
        let piece_bb = match piece_kind {
            PieceKind::Pawn => board.pawns,
            PieceKind::Knight => board.knights,
            PieceKind::Bishop => board.bishops,
            PieceKind::Rook => board.rooks,
            PieceKind::Queen => board.queens,
            PieceKind::King => board.kings,
        };
        
        let candidates = piece_bb & attacking_pieces;
        if candidates == 0 {
            return None;
        }
        
        // Verifica se alguma dessas peças pode atacar a casa
        for candidate_square in 0..64 {
            let candidate_bb = 1u64 << candidate_square;
            if (candidates & candidate_bb) != 0 {
                if Self::can_piece_attack(board, candidate_square, square, piece_kind, color) {
                    return Some(candidate_square);
                }
            }
        }
        
        None
    }
    
    /// Verifica se uma peça pode atacar uma casa específica usando método do board
    fn can_piece_attack(board: &Board, from: u8, to: u8, _piece_kind: PieceKind, color: Color) -> bool {
        // Simula temporariamente que só essa peça existe para testar ataque
        if let Some(piece) = board.get_piece_at(from) {
            if piece.color == color {
                // Usa o método nativo do board que já implementa todos os casos
                board.is_square_attacked_by(to, color)
            } else {
                false
            }
        } else {
            false
        }
    }
    
    /// Calcula resultado final usando minimax na sequência de capturas
    fn minimax_see(gains: &[i32]) -> i32 {
        if gains.is_empty() {
            return 0;
        }
        
        let mut result = gains.to_vec();
        
        // Minimax de trás para frente
        for i in (0..result.len()-1).rev() {
            result[i] = gains[i] - result[i+1];
        }
        
        result[0]
    }
    
    /// Verifica se movimento é captura
    fn is_capture_move(board: &Board, mv: Move) -> bool {
        board.get_piece_at(mv.to).is_some() || mv.is_en_passant
    }
    
    /// Obtém valor material da peça usando método nativo
    fn get_piece_value(piece_kind: PieceKind) -> i32 {
        if piece_kind == PieceKind::King {
            10000 // Valor especial para SEE
        } else {
            piece_kind.value()
        }
    }
    
    /// Avaliação rápida de captura (sem simulação completa)
    pub fn quick_capture_eval(board: &Board, mv: Move) -> i32 {
        if !Self::is_capture_move(board, mv) {
            return 0;
        }
        
        let victim_value = if let Some(victim) = board.get_piece_at(mv.to) {
            Self::get_piece_value(victim.kind)
        } else if mv.is_en_passant {
            100
        } else {
            return 0;
        };
        
        let attacker_value = if let Some(attacker) = board.get_piece_at(mv.from) {
            Self::get_piece_value(attacker.kind)
        } else {
            return 0;
        };
        
        // Avaliação simples: se vítima vale mais, provavelmente boa
        if victim_value >= attacker_value {
            victim_value - attacker_value + 50
        } else if victim_value + 200 < attacker_value {
            victim_value - attacker_value - 100
        } else {
            0
        }
    }
}