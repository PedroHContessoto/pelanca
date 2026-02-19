// Ficheiro: src/engine/perft.rs
// Descrição: Funções PERFT (Performance Test) para validação de geração de movimentos.

use crate::core::board::Board;
use crate::core::types::Move;
use super::perft_tt::PerftTT;
use rayon::prelude::*;

/// PERFT padrão com transposition table interna.
pub fn perft(board: &mut Board, depth: u8) -> u64 {
    perft_with_tt(board, depth, &mut PerftTT::new())
}

/// PERFT com transposition table externa (para reutilização entre chamadas).
pub fn perft_with_tt(board: &mut Board, depth: u8, tt: &mut PerftTT) -> u64 {
    if depth == 0 {
        return 1;
    }

    // Verifica cache primeiro
    if let Some(cached_nodes) = tt.get(board.zobrist_hash, depth) {
        return cached_nodes;
    }

    let moves = board.generate_all_moves(); // pseudo-legais

    if depth == 1 {
        // Bulk counting: Filtra legais sem make/unmake completo
        let nodes = moves.iter()
            .filter(|&&mv| board.is_legal_move(mv))
            .count() as u64;
        tt.insert(board.zobrist_hash, depth, nodes);
        return nodes;
    }

    let mut nodes = 0;

    for mv in moves {
        let undo_info = board.make_move_with_undo(mv);

        let previous_to_move = !board.to_move;
        if !board.is_king_in_check(previous_to_move) {
            nodes += perft_with_tt(board, depth - 1, tt);
        }

        board.unmake_move(mv, undo_info);
    }

    // Cache resultado
    tt.insert(board.zobrist_hash, depth, nodes);
    nodes
}

/// PERFT paralelo para alta performance em CPUs multi-core.
pub fn perft_parallel(board: &mut Board, depth: u8) -> u64 {
    if depth <= 2 {
        // Use versão sequencial para profundidades baixas
        return perft(board, depth);
    }

    let moves = board.generate_all_moves();

    moves.par_iter().map(|&mv| {
        let mut board_clone = *board; // Copy barato devido ao trait Copy
        let _undo_info = board_clone.make_move_with_undo(mv);
        let previous_to_move = !board_clone.to_move;

        if !board_clone.is_king_in_check(previous_to_move) {
            perft_with_tt(&mut board_clone, depth - 1, &mut PerftTT::new())
        } else {
            0
        }
    }).sum()
}

/// Divide PERFT — mostra contagem de nós por lance raiz.
/// Útil para debugging de geração de movimentos.
pub fn perft_divide(board: &mut Board, depth: u8) -> Vec<(Move, u64)> {
    let moves = board.generate_all_moves();
    let mut results = Vec::new();
    let mut tt = PerftTT::new();

    for mv in moves {
        let undo_info = board.make_move_with_undo(mv);
        let previous_to_move = !board.to_move;

        if !board.is_king_in_check(previous_to_move) {
            let nodes = if depth > 1 {
                perft_with_tt(board, depth - 1, &mut tt)
            } else {
                1
            };
            results.push((mv, nodes));
        }

        board.unmake_move(mv, undo_info);
    }

    results
}
