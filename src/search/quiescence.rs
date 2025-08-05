// Quiescence Search - Busca de estabilização para evitar "horizon effect"
// Continua buscando apenas capturas at� posi��o "quieta" (sem capturas pendentes)

use crate::core::*;
use crate::search::{evaluation::Evaluator, move_ordering::MoveOrderer, transposition_table::*};
use std::sync::Arc;

/// Profundidade m�xima para quiescence search
const MAX_QUIESCENCE_DEPTH: i8 = 10;

/// Delta pruning threshold - n�o considera capturas pequenas se posi��o est� muito ruim
const DELTA_PRUNING_MARGIN: i16 = 200;

/// Futility pruning para quiescence - ignora capturas pequenas em posi��es ruins
const FUTILITY_MARGIN: i16 = 150;

/// Estrutura para busca de quiescence
pub struct QuiescenceSearcher {
    pub nodes_searched: u64,
    move_orderer: MoveOrderer,
}

impl QuiescenceSearcher {
    pub fn new() -> Self {
        QuiescenceSearcher {
            nodes_searched: 0,
            move_orderer: MoveOrderer::new(),
        }
    }

    /// Busca principal de quiescence
    pub fn search(
        &mut self,
        board: &mut Board,
        mut alpha: i16,
        beta: i16,
        depth: i8,
        ply: u16,
        tt: Option<&Arc<TranspositionTable>>,
    ) -> i16 {
        self.nodes_searched += 1;

        // Verifica limites de profundidade
        if depth <= -MAX_QUIESCENCE_DEPTH {
            return Evaluator::evaluate(board);
        }

        // Verifica draw por repeti��o ou 50 movimentos
        if board.is_draw_by_50_moves() {
            return 0;
        }

        // Probe da tabela de transposi��o
        if let Some(tt_ref) = tt {
            if let Some(tt_entry) = tt_ref.probe(board.zobrist_hash) {
                if tt_entry.get_depth() >= depth as u8 {
                    let tt_score = adjust_mate_score(tt_entry.get_score(), ply);
                    match tt_entry.get_type() {
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
            }
        }

        // Avalia��o est�tica (stand pat)
        let static_eval = Evaluator::evaluate(board);
        
        // Stand pat: se posi��o j� � boa o suficiente, n�o precisa capturar
        if static_eval >= beta {
            return beta; // Beta cutoff
        }
        
        // Atualiza alpha se necess�rio
        if static_eval > alpha {
            alpha = static_eval;
        }

        // Delta pruning: se mesmo capturando a rainha n�o melhoraria alpha, para
        if static_eval + 900 + DELTA_PRUNING_MARGIN < alpha && depth < 0 {
            return static_eval;
        }

        // Gera apenas movimentos de captura
        let captures = self.generate_captures(board);
        
        if captures.is_empty() {
            return static_eval; // Posi��o quieta
        }

        // Ordena capturas por MVV-LVA
        let mut ordered_captures = captures;
        self.move_orderer.order_moves(board, &mut ordered_captures, None, ply);

        let mut best_score = static_eval;
        let mut node_type = NodeType::UpperBound;
        let mut best_move = ordered_captures[0]; // Fallback

        // Loop principal de busca
        for (move_index, &mv) in ordered_captures.iter().enumerate() {
            // Futility pruning: ignora capturas pequenas em posi��es ruins
            if depth < 0 && move_index > 0 {
                let capture_value = self.estimate_capture_value(board, mv);
                if static_eval + capture_value + FUTILITY_MARGIN < alpha {
                    continue;
                }
            }

            // SEE pruning: ignora capturas claramente perdedoras
            if depth < -2 && self.is_losing_capture(board, mv) {
                continue;
            }

            // Faz o movimento
            let undo_info = board.make_move_with_undo(mv);
            let previous_to_move = !board.to_move;
            
            // Verifica se movimento � legal
            if board.is_king_in_check(previous_to_move) {
                board.unmake_move(mv, undo_info);
                continue;
            }

            // Busca recursiva
            let score = -self.search(board, -beta, -alpha, depth - 1, ply + 1, tt);
            
            // Desfaz movimento
            board.unmake_move(mv, undo_info);

            // Atualiza melhor score
            if score > best_score {
                best_score = score;
                best_move = mv;
                
                if score > alpha {
                    alpha = score;
                    node_type = NodeType::Exact;
                    
                    // Beta cutoff
                    if score >= beta {
                        node_type = NodeType::LowerBound;
                        break;
                    }
                }
            }
        }

        // Armazena resultado na TT
        if let Some(tt_ref) = tt {
            let tt_score = unadjust_mate_score(best_score, ply);
            tt_ref.store(board.zobrist_hash, best_move, tt_score, (-depth) as u8, node_type);
        }

        best_score
    }

    /// Gera apenas movimentos de captura e promo��es
    fn generate_captures(&self, board: &Board) -> Vec<Move> {
        board.generate_all_attacks()
    }


    /// Estima valor aproximado da captura
    fn estimate_capture_value(&self, board: &Board, mv: Move) -> i16 {
        if mv.is_en_passant {
            return 100; // Valor do pe�o
        }
        
        if let Some(promotion) = mv.promotion {
            let promo_value = match promotion {
                PieceKind::Queen => 900,
                PieceKind::Rook => 500,
                PieceKind::Bishop => 330,
                PieceKind::Knight => 320,
                _ => 0,
            };
            return promo_value - 100; // Desconta valor do pe�o promovido
        }
        
        // Valor da pe�a capturada
        self.get_piece_value_at_square(board, mv.to)
    }

    /// Verifica se captura � claramente perdedora (SEE negativo)
    fn is_losing_capture(&self, board: &Board, mv: Move) -> bool {
        let attacker_value = self.get_piece_value_at_square(board, mv.from);
        let victim_value = if mv.is_en_passant { 100 } else { self.get_piece_value_at_square(board, mv.to) };
        
        // Heur�stica simples: se atacante vale muito mais que v�tima e casa est� defendida
        if attacker_value > victim_value + 200 {
            return board.is_square_attacked_by(mv.to, !board.to_move);
        }
        
        false
    }

    /// Obt�m valor da pe�a em uma casa espec�fica
    fn get_piece_value_at_square(&self, board: &Board, square: u8) -> i16 {
        let bb = 1u64 << square;
        
        if (board.pawns & bb) != 0 { 100 }
        else if (board.knights & bb) != 0 { 320 }
        else if (board.bishops & bb) != 0 { 330 }
        else if (board.rooks & bb) != 0 { 500 }
        else if (board.queens & bb) != 0 { 900 }
        else if (board.kings & bb) != 0 { 20000 }
        else { 0 }
    }

    /// Limpa estat�sticas
    pub fn clear_stats(&mut self) {
        self.nodes_searched = 0;
    }

    /// Retorna estat�sticas
    pub fn get_stats(&self) -> (u64,) {
        (self.nodes_searched,)
    }
}

impl Default for QuiescenceSearcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Fun��o auxiliar para busca de quiescence sem estado
pub fn quiescence_search(
    board: &mut Board,
    alpha: i16,
    beta: i16,
    depth: i8,
    ply: u16,
    tt: Option<&Arc<TranspositionTable>>,
) -> i16 {
    let mut qsearcher = QuiescenceSearcher::new();
    qsearcher.search(board, alpha, beta, depth, ply, tt)
}