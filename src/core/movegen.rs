// Ficheiro: src/core/movegen.rs
// Descrição: Geração de movimentos pseudo-legais e legais para o Board.

use super::board::Board;
use super::types::*;
use crate::moves;

impl Board {
    /// Gera todos os lances pseudo-legais para todas as peças do jogador atual (ULTRA-OTIMIZADO)
    pub fn generate_all_moves(&self) -> Vec<Move> {
        // Pre-aloca com capacidade otimizada para posições médias
        let mut moves = Vec::with_capacity(100);

        // OTIMIZAÇÃO CRÍTICA: Eliminados TODOS os Vec::extend!
        moves::pawn::generate_pawn_moves_into(self, &mut moves);
        moves::knight::generate_knight_moves_into(self, &mut moves);
        self.generate_sliding_moves(&mut moves);
        moves::queen::generate_queen_moves_into(self, &mut moves);
        moves::king::generate_king_moves_into(self, &mut moves);

        moves
    }

    /// Gera apenas movimentos legais (filtra movimentos que deixam o rei em xeque)
    pub fn generate_legal_moves(&self) -> Vec<Move> {
        let pseudo_legal = self.generate_all_moves();
        pseudo_legal.into_iter()
            .filter(|&mv| {
                let mut temp = *self;
                temp.make_move(mv);
                !temp.is_king_in_check(self.to_move)
            })
            .collect()
    }

    /// Verifica se um movimento é legal
    pub fn is_legal_move(&self, mv: Move) -> bool {
        let mut temp = *self;
        temp.make_move(mv);
        !temp.is_king_in_check(self.to_move)
    }

    /// Gera apenas capturas pseudo-legais (para quiescence search).
    pub fn generate_capture_moves(&self) -> Vec<Move> {
        let mut moves = Vec::with_capacity(32);
        let our_pieces = if self.to_move == Color::White { self.white_pieces } else { self.black_pieces };
        let enemy_pieces = if self.to_move == Color::White { self.black_pieces } else { self.white_pieces };
        let all_pieces = self.white_pieces | self.black_pieces;

        // Pawn captures (reusa a função existente)
        moves.extend(moves::pawn::generate_pawn_captures(self));

        // Knight captures
        let mut our_knights = self.knights & our_pieces;
        while our_knights != 0 {
            let from_sq = our_knights.trailing_zeros() as u8;
            our_knights &= our_knights - 1;
            let mut captures = moves::knight::get_knight_attacks(from_sq) & enemy_pieces;
            while captures != 0 {
                let to_sq = captures.trailing_zeros() as u8;
                captures &= captures - 1;
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
        }

        // Bishop captures
        let mut our_bishops = self.bishops & our_pieces;
        while our_bishops != 0 {
            let from_sq = our_bishops.trailing_zeros() as u8;
            our_bishops &= our_bishops - 1;
            let attacks = crate::moves::magic_bitboards::get_bishop_attacks_magic(from_sq, all_pieces);
            let mut captures = attacks & enemy_pieces;
            while captures != 0 {
                let to_sq = captures.trailing_zeros() as u8;
                captures &= captures - 1;
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
        }

        // Rook captures
        let mut our_rooks = self.rooks & our_pieces;
        while our_rooks != 0 {
            let from_sq = our_rooks.trailing_zeros() as u8;
            our_rooks &= our_rooks - 1;
            let attacks = crate::moves::magic_bitboards::get_rook_attacks_magic(from_sq, all_pieces);
            let mut captures = attacks & enemy_pieces;
            while captures != 0 {
                let to_sq = captures.trailing_zeros() as u8;
                captures &= captures - 1;
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
        }

        // Queen captures
        let mut our_queens = self.queens & our_pieces;
        while our_queens != 0 {
            let from_sq = our_queens.trailing_zeros() as u8;
            our_queens &= our_queens - 1;
            let attacks = crate::moves::magic_bitboards::get_queen_attacks_magic(from_sq, all_pieces);
            let mut captures = attacks & enemy_pieces;
            while captures != 0 {
                let to_sq = captures.trailing_zeros() as u8;
                captures &= captures - 1;
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
        }

        // King captures (sem roque)
        let our_king = self.kings & our_pieces;
        if our_king != 0 {
            let from_sq = our_king.trailing_zeros() as u8;
            let mut captures = moves::king::get_king_attacks(from_sq) & enemy_pieces;
            while captures != 0 {
                let to_sq = captures.trailing_zeros() as u8;
                captures &= captures - 1;
                moves.push(Move { from: from_sq, to: to_sq, promotion: None, is_castling: false, is_en_passant: false });
            }
        }

        moves
    }

    /// Gera movimentos de peças deslizantes usando magic bitboards diretamente (OTIMIZADO)
    #[inline(always)]
    pub(crate) fn generate_sliding_moves(&self, moves: &mut Vec<Move>) {
        let our_pieces = if self.to_move == Color::White { self.white_pieces } else { self.black_pieces };
        let all_pieces = self.white_pieces | self.black_pieces;

        // Gerar movimentos de bispos
        let mut our_bishops = self.bishops & our_pieces;
        while our_bishops != 0 {
            let from_sq = our_bishops.trailing_zeros() as u8;
            let attacks = crate::moves::magic_bitboards::get_bishop_attacks_magic(from_sq, all_pieces);
            let mut valid_moves = attacks & !our_pieces;

            while valid_moves != 0 {
                let to_sq = valid_moves.trailing_zeros() as u8;
                moves.push(Move {
                    from: from_sq,
                    to: to_sq,
                    promotion: None,
                    is_castling: false,
                    is_en_passant: false
                });
                valid_moves &= valid_moves - 1;
            }

            our_bishops &= our_bishops - 1;
        }

        // Gerar movimentos de torres
        let mut our_rooks = self.rooks & our_pieces;
        while our_rooks != 0 {
            let from_sq = our_rooks.trailing_zeros() as u8;
            let attacks = crate::moves::magic_bitboards::get_rook_attacks_magic(from_sq, all_pieces);
            let mut valid_moves = attacks & !our_pieces;

            while valid_moves != 0 {
                let to_sq = valid_moves.trailing_zeros() as u8;
                moves.push(Move {
                    from: from_sq,
                    to: to_sq,
                    promotion: None,
                    is_castling: false,
                    is_en_passant: false
                });
                valid_moves &= valid_moves - 1;
            }

            our_rooks &= our_rooks - 1;
        }
    }
}
