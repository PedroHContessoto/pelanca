// Ficheiro: src/engine/eval/material.rs
// Descrição: Avaliador simples baseado em contagem de material.

use crate::core::board::Board;
use crate::core::types::{Color, PieceKind};
use crate::engine::traits::{Evaluator, Score};

/// Avaliador de material simples.
/// Score = material_nosso - material_oponente (em centipawns).
/// Usa os valores já definidos em PieceKind::value().
pub struct MaterialEvaluator;

impl Evaluator for MaterialEvaluator {
    fn evaluate(&self, board: &Board) -> Score {
        let side = board.to_move;
        let us = material_sum(board, side);
        let them = material_sum(board, !side);
        us - them
    }

    fn name(&self) -> &str {
        "material"
    }
}

/// Soma o material de uma cor usando piece_count do Board.
#[inline]
fn material_sum(board: &Board, color: Color) -> Score {
    let pieces = [
        PieceKind::Pawn,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Rook,
        PieceKind::Queen,
    ];

    pieces.iter()
        .map(|&kind| board.piece_count(color, kind) as Score * kind.value())
        .sum()
}
