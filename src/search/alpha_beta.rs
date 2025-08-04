// Alpha-Beta Search - Núcleo do motor de xadrez
// Implementação otimizada com técnicas avançadas: TT, Move Ordering, Pruning, etc.

use crate::core::*;
use crate::search::{*, evaluation::Evaluator, move_ordering::MoveOrderer, quiescence::*, transposition_table::*};
use std::sync::Arc;
use std::time::Instant;

/// Valores para detecção de mate
const MATE_SCORE: i16 = 30000;
const MATE_IN_MAX_PLY: i16 = MATE_SCORE - 1000;

/// Limites de busca
const MAX_SEARCH_DEPTH: u8 = 64;
const MAX_PLY: u16 = 128;

/// Margins para pruning
const FUTILITY_MARGIN: [i16; 4] = [0, 200, 300, 500];
const REVERSE_FUTILITY_MARGIN: i16 = 120;
const NULL_MOVE_MARGIN: i16 = 200;
const RAZORING_MARGIN: [i16; 4] = [0, 240, 280, 300];

/// Reduções para Late Move Reduction (LMR)
const LMR_DEPTH_THRESHOLD: u8 = 3;
const LMR_MOVE_THRESHOLD: usize = 4;

/// Estrutura principal do motor de busca Alpha-Beta
pub struct AlphaBetaSearcher {
    pub controller: Arc<SearchController>,
    pub move_orderer: MoveOrderer,
    pub qsearcher: QuiescenceSearcher,
    
    // Estatísticas
    pub nodes_searched: u64,
    pub tt_hits: u64,
    pub tt_misses: u64,
    pub beta_cutoffs: u64,
    pub first_move_cutoffs: u64,
    
    // Melhor movimento encontrado
    pub best_move: Option<Move>,
    pub principal_variation: Vec<Move>,
    
    // Killer moves para cada ply
    killer_moves: [[Option<Move>; 2]; MAX_PLY as usize],
    
    // Counter moves
    counter_moves: [[Option<Move>; 64]; 64],
}

impl AlphaBetaSearcher {
    pub fn new(controller: Arc<SearchController>) -> Self {
        AlphaBetaSearcher {
            controller,
            move_orderer: MoveOrderer::new(),
            qsearcher: QuiescenceSearcher::new(),
            nodes_searched: 0,
            tt_hits: 0,
            tt_misses: 0,
            beta_cutoffs: 0,
            first_move_cutoffs: 0,
            best_move: None,
            principal_variation: Vec::new(),
            killer_moves: [[None; 2]; MAX_PLY as usize],
            counter_moves: [[None; 64]; 64],
        }
    }

    /// Busca iterativa com aprofundamento progressivo
    pub fn iterative_deepening(&mut self, board: &mut Board) -> (Move, SearchStats) {
        let start_time = Instant::now();
        self.clear_search_data();
        
        let mut best_move = None;
        let mut best_score = 0i16;
        let mut depth_completed = 0u8;

        // Obtém lista inicial de movimentos
        let root_moves = board.generate_legal_moves();
        if root_moves.is_empty() {
            // Não há movimentos legais
            let dummy_move = Move {
                from: 0, to: 0, promotion: None,
                is_castling: false, is_en_passant: false,
            };
            return (dummy_move, self.get_search_stats(start_time, depth_completed));
        }

        // Se só há um movimento legal, não precisa buscar muito
        if root_moves.len() == 1 {
            best_move = Some(root_moves[0]);
        }

        // Busca iterativa
        for depth in 1..=self.controller.config.max_depth {
            if self.controller.should_stop() {
                break;
            }

            let iteration_start = Instant::now();
            
            // Aspiration Windows para depths > 4
            let (window_alpha, window_beta) = if depth <= 4 || best_move.is_none() {
                (-MATE_SCORE, MATE_SCORE) // Full window
            } else {
                let aspiration_window = 50;
                (best_score - aspiration_window, best_score + aspiration_window)
            };

            let mut alpha = window_alpha;
            let mut beta = window_beta;
            let mut search_score;

            // Loop de re-search se sair da aspiration window
            loop {
                search_score = self.alpha_beta_root(board, alpha, beta, depth, 0);
                
                if self.controller.should_stop() {
                    break;
                }

                // Verifica se precisa expandir a janela
                if search_score <= alpha {
                    // Fail-low: expande alpha
                    alpha = -MATE_SCORE;
                    if beta != MATE_SCORE {
                        beta = (alpha + beta) / 2;
                    }
                } else if search_score >= beta {
                    // Fail-high: expande beta
                    beta = MATE_SCORE;
                    if alpha != -MATE_SCORE {
                        alpha = (alpha + beta) / 2;
                    }
                } else {
                    // Score dentro da janela
                    break;
                }
            }

            if self.controller.should_stop() {
                break;
            }

            // Atualiza melhor resultado
            best_move = self.best_move;
            best_score = search_score;
            depth_completed = depth;

            // Imprime informações UCI
            let iteration_time = iteration_start.elapsed();
            let nps = if iteration_time.as_secs_f64() > 0.0 {
                (self.nodes_searched as f64 / iteration_time.as_secs_f64()) as u64
            } else {
                0
            };

            println!("info depth {} score cp {} nodes {} nps {} time {} pv {}",
                depth,
                search_score,
                self.nodes_searched,
                nps,
                iteration_time.as_millis(),
                self.format_pv()
            );

            // Para busca antecipada se mate encontrado
            if search_score.abs() > MATE_IN_MAX_PLY {
                break;
            }
        }

        let final_move = best_move.unwrap_or(root_moves[0]);
        let stats = self.get_search_stats(start_time, depth_completed);
        
        (final_move, stats)
    }


    /// Busca Alpha-Beta principal
    pub fn alpha_beta(
        &mut self,
        board: &mut Board,
        mut alpha: i16,
        mut beta: i16,
        mut depth: u8,
        ply: u16,
        is_pv_node: bool,
    ) -> i16 {
        self.nodes_searched += 1;

        // Verifica limites
        if self.controller.should_stop() || ply >= MAX_PLY {
            return Evaluator::evaluate(board);
        }

        // Verifica draws
        if board.is_draw_by_50_moves() || board.is_draw_by_insufficient_material() {
            return 0;
        }

        // Detecção de mate à distância
        let mate_alpha = -MATE_SCORE + ply as i16;
        let mate_beta = MATE_SCORE - ply as i16 - 1;
        if mate_alpha >= beta { return mate_alpha; }
        if mate_beta <= alpha { return mate_beta; }
        alpha = alpha.max(mate_alpha);
        beta = beta.min(mate_beta);

        // Probe da Transposition Table
        let tt_move = if let Ok(tt) = self.controller.tt.lock() {
            if let Some(tt_entry) = tt.probe(board.zobrist_hash) {
                self.tt_hits += 1;
                
                if tt_entry.depth >= depth && !is_pv_node {
                    let tt_score = adjust_mate_score(tt_entry.score, ply);
                    match tt_entry.node_type {
                        NodeType::Exact => return tt_score,
                        NodeType::LowerBound => {
                            if tt_score >= beta {
                                return tt_score;
                            }
                        }
                        NodeType::UpperBound => {
                            if tt_score <= alpha {
                                return tt_score;
                            }
                        }
                    }
                }
                Some(tt_entry.best_move)
            } else {
                self.tt_misses += 1;
                None
            }
        } else {
            self.tt_misses += 1;
            None
        };

        // Quiescence Search na folha
        if depth == 0 {
            return self.qsearcher.search(board, alpha, beta, 0, ply, None);
        }

        let in_check = board.is_king_in_check(board.to_move);
        let static_eval = if in_check {
            -MATE_SCORE + ply as i16 // Em xeque, avaliação pessimista
        } else {
            Evaluator::evaluate(board)
        };

        // Check Extensions
        if in_check {
            depth += 1;
        }

        // Razoring (não PV-nodes)
        if !is_pv_node && !in_check && depth <= 3 {
            if static_eval + RAZORING_MARGIN[depth as usize] < alpha {
                let razoring_score = self.qsearcher.search(board, alpha, beta, 0, ply, None);
                if razoring_score < alpha {
                    return razoring_score;
                }
            }
        }

        // Reverse Futility Pruning (não PV-nodes)
        if !is_pv_node && !in_check && depth <= 6 {
            if static_eval - REVERSE_FUTILITY_MARGIN * (depth as i16) >= beta {
                return static_eval;
            }
        }

        // Null Move Pruning
        if !is_pv_node && !in_check && depth >= 3 && static_eval >= beta {
            let null_reduction = 3 + (depth / 4).min(3) + ((static_eval - beta) / NULL_MOVE_MARGIN).min(3) as u8;
            
            if depth > null_reduction {
                board.to_move = !board.to_move; // Null move
                let null_score = -self.alpha_beta(board, -beta, -beta + 1, depth - null_reduction, ply + 1, false);
                board.to_move = !board.to_move; // Restore
                
                if null_score >= beta {
                    return null_score;
                }
            }
        }

        // Gera e ordena movimentos
        let moves = board.generate_all_moves();
        if moves.is_empty() {
            return if in_check {
                -MATE_SCORE + ply as i16 // Mate
            } else {
                0 // Stalemate
            };
        }

        let mut ordered_moves = moves;
        self.move_orderer.order_moves(board, &mut ordered_moves, tt_move, ply);

        let mut best_score = -MATE_SCORE - 1;
        let mut best_move = ordered_moves[0];
        let mut node_type = NodeType::UpperBound;
        let mut legal_moves = 0;
        let mut quiet_moves = Vec::new();

        // Loop principal de movimentos
        for (move_index, &mv) in ordered_moves.iter().enumerate() {
            let is_capture = self.is_capture_move(board, mv);
            let is_quiet = !is_capture && mv.promotion.is_none();
            
            if is_quiet {
                quiet_moves.push(mv);
            }

            // Late Move Pruning para movimentos silenciosos
            if !is_pv_node && !in_check && is_quiet && depth <= 6 && move_index >= LMR_MOVE_THRESHOLD + (depth as usize * 2) {
                continue;
            }

            // Futility Pruning
            if !is_pv_node && !in_check && is_quiet && depth <= 3 {
                if static_eval + FUTILITY_MARGIN[depth as usize] <= alpha {
                    continue;
                }
            }

            let undo_info = board.make_move_with_undo(mv);
            let previous_to_move = !board.to_move;
            
            // Verifica legalidade
            if board.is_king_in_check(previous_to_move) {
                board.unmake_move(mv, undo_info);
                continue;
            }
            
            legal_moves += 1;

            // Calcula nova profundidade
            let mut new_depth = depth - 1;
            
            // Late Move Reduction (LMR)
            let mut do_reduction = false;
            if !is_pv_node && move_index >= LMR_MOVE_THRESHOLD && depth >= LMR_DEPTH_THRESHOLD && is_quiet {
                let reduction = 1 + (move_index / 6).min(2) + (((depth as usize) - 2) / 4).min(2);
                new_depth = new_depth.saturating_sub(reduction as u8);
                do_reduction = true;
            }

            let score = if move_index == 0 {
                // Primeiro movimento: busca completa
                -self.alpha_beta(board, -beta, -alpha, new_depth, ply + 1, is_pv_node)
            } else {
                // Principal Variation Search (PVS)
                let mut score = -self.alpha_beta(board, -alpha - 1, -alpha, new_depth, ply + 1, false);
                
                // Re-search se necessário
                if do_reduction && score > alpha {
                    score = -self.alpha_beta(board, -alpha - 1, -alpha, depth - 1, ply + 1, false);
                }
                
                if score > alpha && score < beta && is_pv_node {
                    score = -self.alpha_beta(board, -beta, -alpha, depth - 1, ply + 1, true);
                }
                
                score
            };

            board.unmake_move(mv, undo_info);

            if self.controller.should_stop() {
                return best_score;
            }

            if score > best_score {
                best_score = score;
                best_move = mv;

                if score > alpha {
                    alpha = score;
                    node_type = NodeType::Exact;
                    
                    // Atualiza killer moves para movimentos silenciosos
                    if is_quiet && ply < MAX_PLY {
                        self.update_killer_moves(mv, ply);
                    }
                    
                    if score >= beta {
                        // Beta cutoff
                        node_type = NodeType::LowerBound;
                        self.beta_cutoffs += 1;
                        
                        if move_index == 0 {
                            self.first_move_cutoffs += 1;
                        }
                        
                        // Atualiza história
                        self.move_orderer.update_history_cutoff(board, mv, depth, &quiet_moves);
                        
                        // Counter move
                        if ply > 0 && is_quiet {
                            // Implementation would need previous move context
                        }
                        
                        break;
                    }
                }
            }
        }

        if legal_moves == 0 {
            return if in_check {
                -MATE_SCORE + ply as i16 // Mate
            } else {
                0 // Stalemate
            };
        }

        // Armazena na TT
        let tt_score = unadjust_mate_score(best_score, ply);
        if let Ok(tt) = self.controller.tt.lock() {
            tt.store(board.zobrist_hash, best_move, tt_score, depth, node_type);
        }

        best_score
    }

    // Funções auxiliares

    fn is_capture_move(&self, board: &Board, mv: Move) -> bool {
        if mv.is_en_passant {
            return true;
        }
        
        let to_bb = 1u64 << mv.to;
        let enemy_pieces = if board.to_move == Color::White { 
            board.black_pieces 
        } else { 
            board.white_pieces 
        };
        
        (enemy_pieces & to_bb) != 0
    }

    fn update_killer_moves(&mut self, mv: Move, ply: u16) {
        if ply < MAX_PLY {
            let ply_idx = ply as usize;
            
            // Se não é o primeiro killer, move o primeiro para segundo
            if self.killer_moves[ply_idx][0] != Some(mv) {
                self.killer_moves[ply_idx][1] = self.killer_moves[ply_idx][0];
                self.killer_moves[ply_idx][0] = Some(mv);
            }
        }
    }

    fn clear_search_data(&mut self) {
        self.nodes_searched = 0;
        self.tt_hits = 0;
        self.tt_misses = 0;
        self.beta_cutoffs = 0;
        self.first_move_cutoffs = 0;
        self.best_move = None;
        self.principal_variation.clear();
        self.killer_moves = [[None; 2]; MAX_PLY as usize];
        self.qsearcher.clear_stats();
    }

    fn format_pv(&self) -> String {
        if self.principal_variation.is_empty() {
            if let Some(best_move) = self.best_move {
                format!("{}", best_move)
            } else {
                "none".to_string()
            }
        } else {
            self.principal_variation.iter()
                .map(|mv| mv.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        }
    }

    fn get_search_stats(&self, start_time: Instant, depth: u8) -> SearchStats {
        let elapsed = start_time.elapsed();
        let (qnodes,) = self.qsearcher.get_stats();
        
        SearchStats {
            nodes_searched: self.nodes_searched + qnodes,
            tt_hits: self.tt_hits,
            tt_misses: self.tt_misses,
            depth_reached: depth,
            time_elapsed: elapsed,
            nps: if elapsed.as_secs_f64() > 0.0 {
                ((self.nodes_searched + qnodes) as f64 / elapsed.as_secs_f64()) as u64
            } else {
                0
            },
        }
    }
}