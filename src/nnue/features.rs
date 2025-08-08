// Feature extraction otimizada usando bitboards nativos do Pelanca
// Implementa HalfKP com updates incrementais integrados ao make/unmake

use crate::core::{Board, Color, PieceKind, Move, UndoInfo};
use super::{FEATURE_SIZE, HIDDEN_SIZE_1};

/// Acumulador incremental que se integra perfeitamente com make/unmake do Pelanca
#[derive(Debug, Clone)]
pub struct NNUEAccumulator {
    pub accumulator: Vec<i32>,
    pub needs_refresh: bool,
    pub cached_hash: u64,  // Cache baseado no zobrist hash
}

impl NNUEAccumulator {
    pub fn new() -> Self {
        Self {
            accumulator: vec![0; HIDDEN_SIZE_1],
            needs_refresh: true,
            cached_hash: 0,
        }
    }
    
    /// Atualiza features usando as informações do make_move_with_undo
    pub fn update_move(
        &mut self, 
        nnue: &super::NNUE,
        mv: Move, 
        undo_info: &UndoInfo,
        board: &Board
    ) {
        if self.needs_refresh || self.cached_hash != undo_info.old_zobrist_hash {
            self.refresh_full(nnue, board);
            return;
        }
        
        // Update incremental baseado no movimento
        self.apply_move_delta(nnue, mv, undo_info, board);
        self.cached_hash = board.zobrist_hash;
    }
    
    /// Desfaz features usando unmake_move
    pub fn undo_move(&mut self, _nnue: &super::NNUE, _mv: Move, undo_info: &UndoInfo) {
        // Reverte para estado anterior
        self.cached_hash = undo_info.old_zobrist_hash;
        
        // Para simplificidade inicial, marca para refresh completo
        // TODO: Implementar undo incremental verdadeiro
        self.needs_refresh = true;
    }
    
    /// Refresh completo usando bitboards otimizados
    pub fn refresh_full(&mut self, nnue: &super::NNUE, board: &Board) {
        // Limpa acumulador
        for acc in &mut self.accumulator {
            *acc = 0;
        }
        
        // Adiciona bias
        for i in 0..HIDDEN_SIZE_1 {
            self.accumulator[i] = nnue.feature_bias[i];
        }
        
        // Extrai features de cada peça usando bitboards
        self.add_piece_features(nnue, board, Color::White);
        self.add_piece_features(nnue, board, Color::Black);
        
        self.needs_refresh = false;
        self.cached_hash = board.zobrist_hash;
    }
    
    /// Adiciona features de todas as peças de uma cor
    fn add_piece_features(&mut self, nnue: &super::NNUE, board: &Board, color: Color) {
        let color_pieces = if color == Color::White { board.white_pieces } else { board.black_pieces };
        
        // Encontra rei da mesma cor para features HalfKP
        let king_sq = self.find_king_square(board, color);
        if king_sq.is_none() { return; }
        let king_sq = king_sq.unwrap();
        
        // Para cada tipo de peça, percorre bitboard
        self.add_pieces_of_kind(nnue, board.pawns & color_pieces, PieceKind::Pawn, color, king_sq);
        self.add_pieces_of_kind(nnue, board.knights & color_pieces, PieceKind::Knight, color, king_sq);
        self.add_pieces_of_kind(nnue, board.bishops & color_pieces, PieceKind::Bishop, color, king_sq);
        self.add_pieces_of_kind(nnue, board.rooks & color_pieces, PieceKind::Rook, color, king_sq);
        self.add_pieces_of_kind(nnue, board.queens & color_pieces, PieceKind::Queen, color, king_sq);
        // Reis não entram nas features HalfKP
    }
    
    /// Adiciona features de um tipo específico de peça
    fn add_pieces_of_kind(
        &mut self, 
        nnue: &super::NNUE, 
        mut pieces_bb: u64, 
        piece_kind: PieceKind,
        piece_color: Color,
        king_sq: u8
    ) {
        // Percorre bitboard usando trailing_zeros (otimizado)
        while pieces_bb != 0 {
            let sq = pieces_bb.trailing_zeros() as u8;
            pieces_bb &= pieces_bb - 1; // Remove bit menos significativo
            
            let feature_idx = self.calculate_halfkp_index(king_sq, sq, piece_kind, piece_color);
            if feature_idx < FEATURE_SIZE {
                // Adiciona contribuição desta feature ao acumulador
                for i in 0..HIDDEN_SIZE_1 {
                    self.accumulator[i] += nnue.feature_weights[feature_idx * HIDDEN_SIZE_1 + i] as i32;
                }
            }
        }
    }
    
    /// Update incremental baseado no movimento
    fn apply_move_delta(&mut self, nnue: &super::NNUE, mv: Move, undo_info: &UndoInfo, board: &Board) {
        // Remove peça da casa de origem
        let from_king = self.find_king_square_for_piece(board, undo_info.moved_piece, board.to_move);
        if let Some(king_sq) = from_king {
            let from_idx = self.calculate_halfkp_index(king_sq, mv.from, undo_info.moved_piece, !board.to_move);
            self.subtract_feature(nnue, from_idx);
        }
        
        // Adiciona peça na casa de destino (ou peça promovida)
        let piece_kind = if mv.promotion.is_some() { mv.promotion.unwrap() } else { undo_info.moved_piece };
        if let Some(king_sq) = from_king {
            let to_idx = self.calculate_halfkp_index(king_sq, mv.to, piece_kind, !board.to_move);
            self.add_feature(nnue, to_idx);
        }
        
        // Remove peça capturada se houver
        if let Some(captured) = undo_info.captured_piece {
            let captured_king = self.find_king_square_for_piece(board, captured, board.to_move);
            if let Some(king_sq) = captured_king {
                let captured_idx = self.calculate_halfkp_index(king_sq, undo_info.captured_square, captured, board.to_move);
                self.subtract_feature(nnue, captured_idx);
            }
        }
    }
    
    /// Adiciona uma feature ao acumulador
    fn add_feature(&mut self, nnue: &super::NNUE, feature_idx: usize) {
        if feature_idx < FEATURE_SIZE {
            for i in 0..HIDDEN_SIZE_1 {
                self.accumulator[i] += nnue.feature_weights[feature_idx * HIDDEN_SIZE_1 + i] as i32;
            }
        }
    }
    
    /// Remove uma feature do acumulador
    fn subtract_feature(&mut self, nnue: &super::NNUE, feature_idx: usize) {
        if feature_idx < FEATURE_SIZE {
            for i in 0..HIDDEN_SIZE_1 {
                self.accumulator[i] -= nnue.feature_weights[feature_idx * HIDDEN_SIZE_1 + i] as i32;
            }
        }
    }
    
    /// Calcula índice HalfKP otimizado
    fn calculate_halfkp_index(&self, king_sq: u8, piece_sq: u8, piece_kind: PieceKind, piece_color: Color) -> usize {
        // Encoding simplificado: [piece_type][piece_color][king_bucket][piece_square]
        let piece_type_idx = match piece_kind {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5, // Não usado, mas mantido
        };
        
        let piece_color_idx = match piece_color {
            Color::White => 0,
            Color::Black => 1,
        };
        
        // King bucket simplificado (8 buckets por fileira)
        let king_bucket = (king_sq / 8) as usize;
        
        piece_type_idx * 128 + piece_color_idx * 64 + king_bucket * 8 + (piece_sq % 8) as usize
    }
    
    /// Encontra casa do rei usando bitboard otimizado
    fn find_king_square(&self, board: &Board, color: Color) -> Option<u8> {
        let color_pieces = if color == Color::White { board.white_pieces } else { board.black_pieces };
        let king_bb = board.kings & color_pieces;
        
        if king_bb == 0 {
            None
        } else {
            Some(king_bb.trailing_zeros() as u8)
        }
    }
    
    /// Helper para encontrar rei correto baseado na peça
    fn find_king_square_for_piece(&self, board: &Board, _piece: PieceKind, color: Color) -> Option<u8> {
        self.find_king_square(board, color)
    }
}

impl Default for NNUEAccumulator {
    fn default() -> Self {
        Self::new()
    }
}