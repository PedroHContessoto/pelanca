// Ficheiro: src/core/check.rs
// Descrição: Detecção de xeque, ataques e condições de fim de jogo.

use super::board::Board;
use super::types::*;

impl Board {
    /// Verifica se o rei da cor especificada está em xeque (usa cache)
    pub fn is_king_in_check(&self, color: Color) -> bool {
        if color == Color::White {
            self.white_king_in_check
        } else {
            self.black_king_in_check
        }
    }

    /// Atualiza o cache de estado de xeque para ambos os reis
    pub(crate) fn update_check_cache(&mut self) {
        self.white_king_in_check = self.compute_king_in_check(Color::White);
        self.black_king_in_check = self.compute_king_in_check(Color::Black);
    }

    /// Calcula se o rei da cor especificada está em xeque (sem usar cache)
    pub(crate) fn compute_king_in_check(&self, color: Color) -> bool {
        // Encontra a posição do rei
        let king_bb = self.kings & if color == Color::White { self.white_pieces } else { self.black_pieces };
        if king_bb == 0 { return false; } // Não há rei (situação anormal)

        let king_square = king_bb.trailing_zeros() as u8;

        // Verifica se alguma peça inimiga pode atacar o rei
        self.is_square_attacked_by(king_square, !color)
    }

    /// Verifica se uma casa é atacada por peças da cor especificada
    pub fn is_square_attacked_by(&self, square: u8, attacking_color: Color) -> bool {
        let _square_bb = 1u64 << square;
        let attacking_pieces = if attacking_color == Color::White { self.white_pieces } else { self.black_pieces };

        // Early exit: se não há peças atacantes, não há ataques
        if attacking_pieces == 0 { return false; }

        // Verifica ataques de peões usando reverse attacks (O(1) em vez de O(n))
        if (self.pawns & attacking_pieces) != 0 {
            let pawn_attackers = crate::moves::pawn::get_pawn_attackers(square, attacking_color);
            if (pawn_attackers & self.pawns & attacking_pieces) != 0 { return true; }
        }

        // Verifica ataques de cavalos
        if (self.knights & attacking_pieces) != 0 {
            let knight_attacks = crate::moves::knight::get_knight_attacks(square);
            if (knight_attacks & self.knights & attacking_pieces) != 0 { return true; }
        }

        // Verifica ataques do rei
        if (self.kings & attacking_pieces) != 0 {
            let king_attacks = crate::moves::king::get_king_attacks(square);
            if (king_attacks & self.kings & attacking_pieces) != 0 { return true; }
        }

        // Verifica ataques de peças deslizantes usando magic bitboards
        let all_pieces = self.white_pieces | self.black_pieces;

        // Ataques de rainha (combinação de torre + bispo)
        if (self.queens & attacking_pieces) != 0 {
            let queen_attacks = crate::moves::queen::get_queen_attacks(square, all_pieces);
            if (queen_attacks & self.queens & attacking_pieces) != 0 { return true; }
        }

        // Ataques de bispo
        if (self.bishops & attacking_pieces) != 0 {
            let bishop_attacks = crate::moves::magic_bitboards::get_bishop_attacks_magic(square, all_pieces);
            if (bishop_attacks & self.bishops & attacking_pieces) != 0 { return true; }
        }

        // Ataques de torre
        if (self.rooks & attacking_pieces) != 0 {
            let rook_attacks = crate::moves::magic_bitboards::get_rook_attacks_magic(square, all_pieces);
            if (rook_attacks & self.rooks & attacking_pieces) != 0 { return true; }
        }

        false
    }

    /// Verifica se a posição atual é xeque-mate
    pub fn is_checkmate(&self) -> bool {
        if !self.is_king_in_check(self.to_move) {
            return false;
        }

        let moves = self.generate_all_moves();
        moves.iter().all(|&mv| {
            let mut temp = *self;
            temp.make_move(mv);
            temp.is_king_in_check(self.to_move)
        })
    }

    /// Verifica se a posição atual é empate por afogamento
    pub fn is_stalemate(&self) -> bool {
        if self.is_king_in_check(self.to_move) {
            return false;
        }

        let moves = self.generate_all_moves();
        moves.iter().all(|&mv| {
            let mut temp = *self;
            temp.make_move(mv);
            temp.is_king_in_check(self.to_move)
        })
    }

    /// Verifica se há empate por material insuficiente
    pub fn is_draw_by_insufficient_material(&self) -> bool {
        let total_pieces = self.white_pieces | self.black_pieces;
        let piece_count = total_pieces.count_ones();

        // King vs King
        if piece_count == 2 {
            return true;
        }

        // King + minor piece vs King
        if piece_count == 3 {
            let has_major_pieces = (self.pawns | self.rooks | self.queens) != 0;
            if !has_major_pieces {
                let minors = self.knights | self.bishops;
                return minors.count_ones() == 1;
            }
        }

        // King + Bishop vs King + Bishop (same color squares)
        if piece_count == 4 && (self.pawns | self.rooks | self.queens | self.knights) == 0 {
            let white_bishops = self.bishops & self.white_pieces;
            let black_bishops = self.bishops & self.black_pieces;

            if white_bishops.count_ones() == 1 && black_bishops.count_ones() == 1 {
                let light_squares = 0x55AA55AA55AA55AA;
                let white_on_light = (white_bishops & light_squares) != 0;
                let black_on_light = (black_bishops & light_squares) != 0;
                return white_on_light == black_on_light;
            }
        }

        false
    }

    /// Verifica se há empate pela regra dos 50 movimentos
    pub fn is_draw_by_50_moves(&self) -> bool {
        self.halfmove_clock >= 100 // 50 movimentos = 100 half-moves
    }

    /// Verifica se o jogo acabou (xeque-mate ou empate)
    pub fn is_game_over(&self) -> bool {
        self.is_checkmate() || self.is_stalemate() || self.is_draw_by_insufficient_material() || self.is_draw_by_50_moves()
    }
}
