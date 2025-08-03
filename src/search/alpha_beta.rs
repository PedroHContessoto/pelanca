// Implementação do algoritmo Alpha-Beta com otimizações modernas

use super::*;
use crate::core::*;
use std::sync::Arc;
use rayon::prelude::*;

/// Busca principal com iterative deepening e aspiration windows
pub fn search(board: &mut Board, controller: Arc<SearchController>) -> (Move, SearchStats) {
    let mut stats = SearchStats::new();
    let start_time = controller.start_time;

    // Melhor movimento encontrado até agora
    let mut best_move = Move { from: 0, to: 0, promotion: None, is_castling: false, is_en_passant: false };
    let mut best_score = -INF;

    // Iterative deepening
    for depth in 1..=controller.config.max_depth {
        if controller.should_stop() {
            break;
        }

        // Aspiration windows
        let mut alpha = if depth > 3 && best_score.abs() < MATE_THRESHOLD {
            best_score - controller.config.aspiration_window
        } else {
            -INF
        };

        let mut beta = if depth > 3 && best_score.abs() < MATE_THRESHOLD {
            best_score + controller.config.aspiration_window
        } else {
            INF
        };

        // Busca com janela de aspiração
        loop {
            let (mv, score, pv) = if controller.config.num_threads > 1 && depth > 4 {
                search_root_parallel(board, depth, alpha, beta, &controller)
            } else {
                search_root(board, depth, alpha, beta, &controller)
            };

            // Verifica se precisa re-buscar com janela maior
            if score <= alpha && score > -MATE_THRESHOLD {
                alpha = -INF;
                continue;
            } else if score >= beta && score < MATE_THRESHOLD {
                beta = INF;
                continue;
            }

            // Busca completa
            best_move = mv;
            best_score = score;
            stats.principal_variation = pv;
            break;
        }

        stats.depth_reached = depth;

        // Log UCI
        let elapsed = controller.get_elapsed();
        let nps = if elapsed.as_secs_f64() > 0.0 {
            (controller.node_counter.load(std::sync::atomic::Ordering::Relaxed) as f64 / elapsed.as_secs_f64()) as u64
        } else {
            0
        };

        println!("info depth {} score cp {} nodes {} nps {} time {} pv {}",
                 depth,
                 best_score,
                 controller.node_counter.load(std::sync::atomic::Ordering::Relaxed),
                 nps,
                 elapsed.as_millis(),
                 pv_to_string(&stats.principal_variation)
        );

        // Early exit se encontrou mate
        if best_score.abs() >= MATE_THRESHOLD {
            break;
        }
    }

    // Atualiza estatísticas finais
    stats.nodes_searched = controller.node_counter.load(std::sync::atomic::Ordering::Relaxed);
    stats.elapsed_time = controller.get_elapsed();
    let tt_stats = controller.tt.get_stats();
    stats.tt_hits = tt_stats.0;
    stats.tt_misses = tt_stats.1;

    (best_move, stats)
}

/// Busca na raiz da árvore (single-threaded)
pub fn search_root(board: &mut Board, depth: u8, mut alpha: i32, beta: i32, controller: &Arc<SearchController>) -> (Move, i32, Vec<Move>) {
    let mut moves = board.generate_all_moves();
    order_moves(board, &mut moves, None, 0, controller);

    let mut best_move = moves[0];
    let mut best_score = -INF;
    let mut pv = Vec::new();

    for (i, &mv) in moves.iter().enumerate() {
        let undo_info = board.make_move_with_undo(mv);

        if !board.is_king_in_check(!board.to_move) {
            controller.increment_nodes(1);

            // Primeira jogada com janela completa, demais com janela nula
            let score = if i == 0 {
                -alpha_beta(board, depth - 1, -beta, -alpha, 1, controller, NodeType::PV)
            } else {
                // Busca com janela nula
                let null_score = -alpha_beta(board, depth - 1, -alpha - 1, -alpha, 1, controller, NodeType::Cut);

                if null_score > alpha && null_score < beta {
                    // Re-busca com janela completa se falhou
                    -alpha_beta(board, depth - 1, -beta, -alpha, 1, controller, NodeType::PV)
                } else {
                    null_score
                }
            };

            if score > best_score {
                best_score = score;
                best_move = mv;

                // Extrai variação principal
                pv = vec![mv];
                extract_pv_from_tt(board, depth - 1, &mut pv, controller);
            }

            alpha = alpha.max(score);
        }

        board.unmake_move(mv, undo_info);

        if controller.should_stop() {
            break;
        }
    }

    (best_move, best_score, pv)
}

/// Busca paralela na raiz (multi-threaded)
fn search_root_parallel(board: &mut Board, depth: u8, alpha: i32, beta: i32, controller: &Arc<SearchController>) -> (Move, i32, Vec<Move>) {
    let mut moves = board.generate_all_moves();
    order_moves(board, &mut moves, None, 0, controller);

    // Filtra movimentos legais
    let legal_moves: Vec<Move> = moves.iter()
        .filter(|&&mv| {
            let mut temp_board = *board;
            temp_board.make_move(mv);
            !temp_board.is_king_in_check(!temp_board.to_move)
        })
        .copied()
        .collect();

    if legal_moves.is_empty() {
        return (moves[0], -INF, Vec::new());
    }

    // Busca paralela
    let results: Vec<(Move, i32, Vec<Move>)> = legal_moves.par_iter()
        .map(|&mv| {
            let mut board_clone = *board;
            let mut local_pv = vec![mv];

            board_clone.make_move(mv);
            controller.increment_nodes(1);

            let score = -alpha_beta(&mut board_clone, depth - 1, -beta, -alpha, 1, controller, NodeType::PV);

            // Extrai PV local
            extract_pv_from_tt(&mut board_clone, depth - 1, &mut local_pv, controller);

            (mv, score, local_pv)
        })
        .collect();

    // Encontra melhor resultado
    results.into_iter()
        .max_by_key(|(_, score, _)| *score)
        .unwrap_or((legal_moves[0], -INF, Vec::new()))
}

/// Algoritmo Alpha-Beta principal com todas as otimizações
fn alpha_beta(
    board: &mut Board,
    mut depth: u8,
    mut alpha: i32,
    mut beta: i32,
    ply: i32,
    controller: &Arc<SearchController>,
    node_type: NodeType,
) -> i32 {
    // Verifica parada
    if controller.should_stop() {
        return 0;
    }

    controller.increment_nodes(1);

    // Verifica empate por repetição ou 50 movimentos
    if board.is_draw_by_50_moves() || is_repetition(board) {
        return DRAW_SCORE;
    }

    // Busca na transposition table
    let tt_entry = controller.tt.probe(board.zobrist_hash);
    let mut tt_move = None;

    if let Some(entry) = tt_entry {
        tt_move = Some(entry.best_move);

        if entry.depth >= depth {
            let score = entry.score;

            match entry.flag {
                TTFlag::Exact => return mate_score_adjustment(score as i32, ply),
                TTFlag::LowerBound => {
                    alpha = alpha.max(score as i32);
                    if alpha >= beta {
                        return mate_score_adjustment(score as i32, ply);
                    }
                }
                TTFlag::UpperBound => {
                    beta = beta.min(score as i32);
                    if alpha >= beta {
                        return mate_score_adjustment(score as i32, ply);
                    }
                }
            }
        }
    }

    // Profundidade 0 ou busca de quiescência
    if depth == 0 {
        if controller.config.use_quiescence {
            return quiescence_search(board, alpha, beta, ply, controller);
        } else {
            return evaluate(board);
        }
    }

    // Null move pruning (não em finais ou quando em xeque)
    if depth >= 3 &&
        node_type != NodeType::PV &&
        !board.is_king_in_check(board.to_move) &&
        !is_endgame(board) {

        // Faz null move (passa a vez)
        board.to_move = !board.to_move;
        board.zobrist_hash ^= crate::core::zobrist::ZOBRIST_KEYS.side_to_move;

        let null_score = -alpha_beta(board, depth.saturating_sub(3), -beta, -beta + 1, ply + 1, controller, NodeType::All);

        // Desfaz null move
        board.to_move = !board.to_move;
        board.zobrist_hash ^= crate::core::zobrist::ZOBRIST_KEYS.side_to_move;

        if null_score >= beta {
            return beta;
        }
    }

    // Gera e ordena movimentos
    let mut moves = board.generate_all_moves();
    order_moves(board, &mut moves, tt_move, ply, controller);

    let mut best_move = moves[0];
    let mut best_score = -INF;
    let mut moves_searched = 0;
    let original_alpha = alpha;

    // Late move reductions
    let can_reduce = depth >= 3 && node_type != NodeType::PV;

    for (i, &mv) in moves.iter().enumerate() {
        let undo_info = board.make_move_with_undo(mv);

        if !board.is_king_in_check(!board.to_move) {
            moves_searched += 1;

            let mut score;

            // Principal variation search
            if moves_searched == 1 {
                score = -alpha_beta(board, depth - 1, -beta, -alpha, ply + 1, controller, NodeType::PV);
            } else {
                // Late move reduction
                let reduction = if can_reduce &&
                    moves_searched > 4 &&
                    !is_capture(board, mv) &&
                    !board.is_king_in_check(board.to_move) {
                    1 + (moves_searched > 8) as u8
                } else {
                    0
                };

                // Busca com janela nula
                score = -alpha_beta(board, depth.saturating_sub(1 + reduction), -alpha - 1, -alpha, ply + 1, controller, NodeType::Cut);

                // Re-busca se necessário
                if score > alpha && score < beta {
                    score = -alpha_beta(board, depth - 1, -beta, -alpha, ply + 1, controller, NodeType::PV);
                }
            }

            if score > best_score {
                best_score = score;
                best_move = mv;
            }

            alpha = alpha.max(score);

            // Beta cutoff
            if alpha >= beta {
                // Atualiza killer moves
                update_killers(mv, ply, controller);

                // Armazena na TT
                controller.tt.store(
                    board.zobrist_hash,
                    depth,
                    score,
                    TTFlag::LowerBound,
                    best_move,
                );

                return beta;
            }
        }

        board.unmake_move(mv, undo_info);
    }

    // Verifica mate ou pat
    if moves_searched == 0 {
        if board.is_king_in_check(board.to_move) {
            return -MATE_SCORE + ply; // Mate
        } else {
            return DRAW_SCORE; // Pat
        }
    }

    // Armazena na transposition table
    let flag = if best_score <= original_alpha {
        TTFlag::UpperBound
    } else if best_score >= beta {
        TTFlag::LowerBound
    } else {
        TTFlag::Exact
    };

    controller.tt.store(
        board.zobrist_hash,
        depth,
        best_score,
        flag,
        best_move,
    );

    best_score
}

// Funções auxiliares

fn is_capture(board: &Board, mv: Move) -> bool {
    let to_bb = 1u64 << mv.to;
    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };
    (enemy_pieces & to_bb) != 0 || mv.is_en_passant
}

fn is_endgame(board: &Board) -> bool {
    let total_pieces = (board.white_pieces | board.black_pieces).count_ones();
    total_pieces <= 10
}

fn is_repetition(_board: &Board) -> bool {
    // Simplificado - idealmente manteria histórico de posições
    false
}

fn pv_to_string(pv: &[Move]) -> String {
    pv.iter()
        .map(|mv| mv.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_pv_from_tt(board: &mut Board, mut depth: u8, pv: &mut Vec<Move>, controller: &Arc<SearchController>) {
    while depth > 0 && pv.len() < 20 {
        if let Some(entry) = controller.tt.probe(board.zobrist_hash) {
            let mv = entry.best_move;

            // Verifica se o movimento é legal
            let moves = board.generate_all_moves();
            if !moves.contains(&mv) {
                break;
            }

            let undo_info = board.make_move_with_undo(mv);
            if board.is_king_in_check(!board.to_move) {
                board.unmake_move(mv, undo_info);
                break;
            }

            pv.push(mv);
            depth = depth.saturating_sub(1);
        } else {
            break;
        }
    }

    // Desfaz todos os movimentos
    for &mv in pv.iter().skip(1).rev() {
        // Nota: seria melhor manter os undo_infos, mas simplificado aqui
        let moves = board.generate_all_moves();
        if let Some(&original_mv) = moves.iter().find(|&&m| m == mv) {
            let undo_info = board.make_move_with_undo(original_mv);
            board.unmake_move(original_mv, undo_info);
        }
    }
}

// Placeholder para killer moves (seria melhor em estrutura separada)
fn update_killers(_mv: Move, _ply: i32, _controller: &Arc<SearchController>) {
    // Implementação futura
}