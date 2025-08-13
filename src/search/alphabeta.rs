use crate::core::*;
use super::*;
use std::time::{Instant, Duration};

/// Search Engine principal com Alpha-Beta + Quiescence
pub struct SearchEngine {
    evaluator: Evaluator,
    tt: TranspositionTable,
    move_orderer: MoveOrderer,
    
    // Estatísticas
    nodes_searched: u64,
    start_time: Instant,
    time_limit: Option<Duration>,
    
    // Killer moves por profundidade
    killer_moves: [[Option<Move>; 2]; MAX_PLY],
    
    // History heuristic [from][to]
    history: [[i32; 64]; 64],
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            evaluator: Evaluator::new(),
            tt: TranspositionTable::new(),
            move_orderer: MoveOrderer::new(),
            nodes_searched: 0,
            start_time: Instant::now(),
            time_limit: None,
            killer_moves: [[None; 2]; MAX_PLY],
            history: [[0; 64]; 64],
        }
    }

    /// Busca principal - interface pública
    pub fn search(&mut self, board: &mut Board, max_depth: Depth) -> SearchResult {
        self.reset_search();
        self.start_time = Instant::now();
        
        let mut best_move = None;
        let mut best_score = -MATE_SCORE;
        let mut pv = Vec::new();
        let mut prev_score = 0;

        // Iterative Deepening com aspiration windows
        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }

            let score = if depth >= 3 && !Evaluator::is_mate_score(prev_score) {
                // Usa aspiration windows para depths >= 3
                self.search_with_aspiration(board, depth, prev_score)
            } else {
                // Primeira busca ou após mate - janela completa
                self.alpha_beta_root(board, depth)
            };
            
            // Busca o melhor movimento na TT
            if let Some(mv) = self.tt.get_best_move(board.zobrist_hash) {
                best_move = Some(mv);
                best_score = score;
                prev_score = score;
            }

            pv.push(SearchInfo {
                depth,
                score,
                nodes: self.nodes_searched,
                time: self.start_time.elapsed(),
                pv: vec![best_move.unwrap_or_else(|| Move { from: 0, to: 0, promotion: None, is_castling: false, is_en_passant: false })],
            });

            // Mate encontrado - para imediatamente
            if Evaluator::is_mate_score(score) {
                println!("  🎯 MATE detectado na depth {}: {} = {}cp", depth, best_move.unwrap(), score);
                break;
            }
        }

        SearchResult {
            best_move,
            score: best_score,
            search_info: pv,
        }
    }

    /// Busca com aspiration windows para melhor performance
    fn search_with_aspiration(&mut self, board: &mut Board, depth: Depth, prev_score: Score) -> Score {
        let mut alpha = prev_score - 50;  // Janela de ±50 centipawns
        let mut beta = prev_score + 50;
        
        loop {
            let score = self.alpha_beta_root_windowed(board, depth, alpha, beta);
            
            if score <= alpha {
                // Fail low - expande janela inferior
                alpha = -MATE_SCORE;
            } else if score >= beta {
                // Fail high - expande janela superior  
                beta = MATE_SCORE;
            } else {
                // Score dentro da janela
                return score;
            }
            
            // Previne loop infinito
            if alpha == -MATE_SCORE && beta == MATE_SCORE {
                return score;
            }
        }
    }

    /// Alpha-beta root com janela específica
    fn alpha_beta_root_windowed(&mut self, board: &mut Board, depth: Depth, mut alpha: Score, beta: Score) -> Score {
        let mut best_move = None;
        let mut legal_moves = 0;

        let moves = board.generate_all_moves();
        let moves = self.move_orderer.order_moves(moves, board, &self.tt, 0);

        for mv in moves {
            if !board.is_legal_move(mv) {
                continue;
            }
            
            legal_moves += 1;

            let undo_info = board.make_move_with_undo(mv);
            let score = -self.alpha_beta(board, depth - 1, -beta, -alpha, 1);
            board.unmake_move(mv, undo_info);

            if score > alpha {
                alpha = score;
                best_move = Some(mv);
                
                if alpha >= beta {
                    break; // Beta cutoff
                }
            }
        }

        // Se não há movimentos legais
        if legal_moves == 0 {
            alpha = if board.is_king_in_check(board.to_move) {
                -MATE_SCORE
            } else {
                0
            };
        }

        // Armazena na TT
        if let Some(mv) = best_move {
            let node_type = if alpha >= beta {
                TTNodeType::Beta
            } else {
                TTNodeType::Exact
            };
            self.tt.store(board.zobrist_hash, depth, alpha, node_type, Some(mv));
        }

        alpha
    }

    /// Alpha-beta na raiz (depth máxima)
    fn alpha_beta_root(&mut self, board: &mut Board, depth: Depth) -> Score {
        let mut alpha = -MATE_SCORE;
        let beta = MATE_SCORE;
        let mut best_move = None;
        let mut legal_moves = 0;

        let moves = board.generate_all_moves();
        let moves = self.move_orderer.order_moves(moves, board, &self.tt, 0);

        for mv in moves {
            if !board.is_legal_move(mv) {
                continue;
            }
            
            legal_moves += 1;

            let undo_info = board.make_move_with_undo(mv);
            let score = -self.alpha_beta(board, depth - 1, -beta, -alpha, 1);
            board.unmake_move(mv, undo_info);

            if score > alpha {
                alpha = score;
                best_move = Some(mv);
                
                // Se encontramos mate, não precisa continuar
                if Evaluator::is_mate_score(score) {
                    break;
                }
            }
        }

        // Se não há movimentos legais
        if legal_moves == 0 {
            alpha = if board.is_king_in_check(board.to_move) {
                -MATE_SCORE // Checkmate
            } else {
                0 // Stalemate
            };
        }

        // Armazena na TT
        if let Some(mv) = best_move {
            self.tt.store(board.zobrist_hash, depth, alpha, TTNodeType::Exact, Some(mv));
        }

        alpha
    }

    /// Alpha-Beta principal
    fn alpha_beta(&mut self, board: &mut Board, depth: Depth, mut alpha: Score, beta: Score, ply: Ply) -> Score {
        self.nodes_searched += 1;

        // Verifica timeout mais frequentemente
        if self.nodes_searched % 1024 == 0 && self.should_stop() {
            return alpha;
        }

        // Detecção rápida de mate/empate
        let in_check = board.is_king_in_check(board.to_move);
        
        // Se estamos em xeque, precisamos verificar se há movimentos legais
        if in_check {
            let legal_moves = board.generate_all_moves()
                .into_iter()
                .filter(|&mv| board.is_legal_move(mv))
                .count();
                
            if legal_moves == 0 {
                return -Evaluator::mate_in_n(ply); // Checkmate
            }
        }

        // Check extension - estende busca se em xeque
        let extended_depth = if in_check && depth == 1 {
            2 // Estende por 1 ply se em xeque
        } else {
            depth
        };

        // Profundidade zero -> Quiescence
        if extended_depth == 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        // Consulta TT
        if let Some(tt_score) = self.tt.probe(board.zobrist_hash, depth, alpha, beta) {
            return TranspositionTable::score_from_tt(tt_score, ply);
        }

        // Null Move Pruning
        if depth >= 3 && !in_check && ply > 0 {
            // Não faz null move se estamos em endgame ou posição crítica
            if !self.is_endgame(board) && alpha > -MATE_IN_MAX && alpha < MATE_IN_MAX {
                // Faz null move
                board.to_move = !board.to_move;
                
                let reduction = if depth > 6 { 4 } else { 3 };
                let null_depth = if depth > reduction { depth - reduction } else { 0 };
                
                let null_score = -self.alpha_beta(board, null_depth, -beta, -beta + 1, ply + 1);
                
                // Desfaz null move
                board.to_move = !board.to_move;
                
                if null_score >= beta {
                    return beta; // Null move cutoff
                }
            }
        }

        let mut moves = board.generate_all_moves();
        
        // Separação e priorização de movimentos forçantes
        let tt_move = self.tt.get_best_move(board.zobrist_hash);
        let has_mate_potential = self.evaluator.has_mate_potential(board);
        let (mut forcing_moves, mut quiet_moves) = self.categorize_moves(&moves, board);
        
        // Ordena movimentos forçantes primeiro - com bonus extra se há potencial de mate
        forcing_moves.sort_unstable_by(|&a, &b| {
            let mut score_a = self.move_orderer.score_move_with_heuristics(
                a, board, tt_move, ply, 
                self.is_killer_move(a, ply), 
                self.get_history_score(a)
            );
            let mut score_b = self.move_orderer.score_move_with_heuristics(
                b, board, tt_move, ply,
                self.is_killer_move(b, ply),
                self.get_history_score(b)
            );
            
            // Bonus leve para capturas se há potencial de mate (sem verificar xeque)
            if has_mate_potential {
                if self.is_capture(a, board) {
                    score_a += 100_000;
                }
                if self.is_capture(b, board) {
                    score_b += 100_000;
                }
            }
            
            score_b.cmp(&score_a)
        });
        
        // Ordena movimentos quietos
        quiet_moves.sort_unstable_by(|&a, &b| {
            let score_a = self.move_orderer.score_move_with_heuristics(
                a, board, tt_move, ply, 
                self.is_killer_move(a, ply), 
                self.get_history_score(a)
            );
            let score_b = self.move_orderer.score_move_with_heuristics(
                b, board, tt_move, ply,
                self.is_killer_move(b, ply),
                self.get_history_score(b)
            );
            score_b.cmp(&score_a)
        });
        
        // Combina: TT move (implícito no score), forcing moves, quiet moves
        moves = forcing_moves;
        moves.extend(quiet_moves);

        let mut legal_moves = 0;
        let mut best_move = None;
        let original_alpha = alpha;

        for (move_count, mv) in moves.iter().enumerate() {
            if !board.is_legal_move(*mv) {
                continue;
            }
            
            legal_moves += 1;

            let undo_info = board.make_move_with_undo(*mv);
            
            let gives_check = self.gives_check(board, *mv);
            let is_capture = self.is_capture(*mv, board);
            
            let score = if move_count >= 4 && extended_depth >= 3 && !in_check && 
                         !is_capture && !gives_check &&
                         !Evaluator::is_mate_score(alpha) {
                // Late Move Reduction (LMR)
                let reduction = if move_count >= 6 { 2 } else { 1 };
                let reduced_depth = if extended_depth > reduction { extended_depth - reduction } else { 1 };
                
                let lmr_score = -self.alpha_beta(board, reduced_depth, -alpha - 1, -alpha, ply + 1);
                
                if lmr_score > alpha {
                    // Re-search com depth completo
                    -self.alpha_beta(board, extended_depth - 1, -beta, -alpha, ply + 1)
                } else {
                    lmr_score
                }
            } else {
                // Busca normal (com check extension se aplicável)
                let next_depth = if gives_check && extended_depth < depth + 2 {
                    extended_depth // Pode estender mais para xeques
                } else {
                    extended_depth - 1
                };
                -self.alpha_beta(board, next_depth, -beta, -alpha, ply + 1)
            };
            
            board.unmake_move(*mv, undo_info);

            if score >= beta {
                // Beta cutoff
                self.update_killer_move(*mv, ply);
                self.update_history(*mv, depth);
                
                let tt_score = TranspositionTable::score_to_tt(beta, ply);
                self.tt.store(board.zobrist_hash, depth, tt_score, TTNodeType::Beta, Some(*mv));
                return beta;
            }

            if score > alpha {
                alpha = score;
                best_move = Some(*mv);
                
                // Se encontramos mate, para imediatamente!
                if Evaluator::is_mate_score(score) {
                    break;
                }
            }
        }

        // Sem movimentos legais
        if legal_moves == 0 {
            return if board.is_king_in_check(board.to_move) {
                -Evaluator::mate_in_n(ply) // Checkmate - perdemos
            } else {
                0 // Stalemate
            };
        }

        // Armazena na TT
        let node_type = if alpha > original_alpha {
            TTNodeType::Exact
        } else {
            TTNodeType::Alpha
        };

        let tt_score = TranspositionTable::score_to_tt(alpha, ply);
        self.tt.store(board.zobrist_hash, depth, tt_score, node_type, best_move);

        alpha
    }

    /// Quiescence Search - busca apenas capturas para evitar horizon effect
    fn quiescence(&mut self, board: &mut Board, mut alpha: Score, beta: Score, ply: Ply) -> Score {
        self.nodes_searched += 1;

        // Limite de profundidade para evitar explosão combinatória
        if ply >= MAX_PLY as u8 - 1 {
            return self.evaluator.evaluate(board);
        }

        // Stand pat - posição atual pode ser boa o suficiente
        let stand_pat = self.evaluator.evaluate(board);
        
        if stand_pat >= beta {
            return beta;
        }
        
        // Delta pruning: se mesmo capturando a dama não melhora alpha, pode parar
        let big_delta = 900; // Valor da dama
        if stand_pat + big_delta < alpha {
            return alpha;
        }
        
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        // Gera apenas capturas e promoções
        let mut captures = self.generate_captures_and_promotions(board);
        
        // Ordena capturas por MVV-LVA
        captures = self.move_orderer.order_captures(captures, board);

        for mv in captures {
            if !board.is_legal_move(mv) {
                continue;
            }

            // SEE (Static Exchange Evaluation) melhorada
            if self.is_losing_capture(board, mv) {
                continue;
            }

            // Delta pruning específico para esta captura
            let captured_value = if let Some(captured) = board.get_piece_at(mv.to) {
                captured.kind.value()
            } else if mv.is_en_passant {
                100 // Valor do peão
            } else if mv.promotion.is_some() {
                800 // Promoção vale muito
            } else {
                continue; // Não é captura nem promoção
            };

            if stand_pat + captured_value + 200 < alpha {
                continue; // Delta pruning
            }

            let undo_info = board.make_move_with_undo(mv);
            let score = -self.quiescence(board, -beta, -alpha, ply + 1);
            board.unmake_move(mv, undo_info);

            if score >= beta {
                return beta;
            }
            
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }

    /// Gera apenas capturas e promoções para quiescence
    fn generate_captures_and_promotions(&self, board: &Board) -> Vec<Move> {
        let all_moves = board.generate_all_moves();
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };

        all_moves.into_iter()
            .filter(|mv| {
                // Capturas
                (1u64 << mv.to) & enemy_pieces != 0 ||
                // En passant
                mv.is_en_passant ||
                // Promoções
                mv.promotion.is_some()
            })
            .collect()
    }

    /// SEE melhorada - determina se uma captura é perdedora
    fn is_losing_capture(&self, board: &Board, mv: Move) -> bool {
        // Para promoções, sempre vale a pena tentar
        if mv.promotion.is_some() {
            return false;
        }

        // En passant é geralmente seguro
        if mv.is_en_passant {
            return false;
        }

        if let Some(captured) = board.get_piece_at(mv.to) {
            if let Some(attacker) = board.get_piece_at(mv.from) {
                // Se capturamos peça mais valiosa, sempre bom
                if captured.kind.value() >= attacker.kind.value() {
                    return false;
                }
                
                // Se a diferença é muito grande (ex: peão captura dama), ruim
                if attacker.kind.value() - captured.kind.value() > 400 {
                    return true;
                }
            }
        }
        false
    }

    fn update_killer_move(&mut self, mv: Move, ply: Ply) {
        let ply_idx = ply as usize;
        if ply_idx < MAX_PLY {
            // Se já é killer move, não atualiza
            if self.killer_moves[ply_idx][0] == Some(mv) {
                return;
            }
            
            // Move killer atual para segunda posição
            self.killer_moves[ply_idx][1] = self.killer_moves[ply_idx][0];
            self.killer_moves[ply_idx][0] = Some(mv);
        }
    }

    fn update_history(&mut self, mv: Move, depth: Depth) {
        let bonus = depth as i32 * depth as i32;
        self.history[mv.from as usize][mv.to as usize] += bonus;
        
        // Decay para evitar overflow
        if self.history[mv.from as usize][mv.to as usize] > 10000 {
            for i in 0..64 {
                for j in 0..64 {
                    self.history[i][j] /= 2;
                }
            }
        }
    }

    pub fn get_history_score(&self, mv: Move) -> i32 {
        self.history[mv.from as usize][mv.to as usize]
    }

    pub fn is_killer_move(&self, mv: Move, ply: u8) -> bool {
        let ply_idx = ply as usize;
        if ply_idx < MAX_PLY {
            self.killer_moves[ply_idx][0] == Some(mv) || 
            self.killer_moves[ply_idx][1] == Some(mv)
        } else {
            false
        }
    }

    fn reset_search(&mut self) {
        self.nodes_searched = 0;
        self.killer_moves = [[None; 2]; MAX_PLY];
        self.history = [[0; 64]; 64];
        self.tt.age();
    }

    fn should_stop(&self) -> bool {
        if let Some(limit) = self.time_limit {
            self.start_time.elapsed() >= limit
        } else {
            false
        }
    }

    pub fn set_time_limit(&mut self, duration: Duration) {
        self.time_limit = Some(duration);
    }

    pub fn get_stats(&self) -> SearchStats {
        SearchStats {
            nodes_searched: self.nodes_searched,
            time_elapsed: self.start_time.elapsed(),
            tt_hit_rate: self.tt.hit_rate(),
            tt_usage: self.tt.usage_percentage(),
        }
    }

    /// Verifica se o movimento é uma captura
    fn is_capture(&self, mv: Move, board: &Board) -> bool {
        let enemy_pieces = if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };
        
        (1u64 << mv.to) & enemy_pieces != 0 || mv.is_en_passant
    }

    /// Detecta se estamos em endgame
    fn is_endgame(&self, board: &Board) -> bool {
        // Simples: endgame se não há damas ou poucas peças
        board.queens == 0 || 
        (board.white_pieces | board.black_pieces).count_ones() <= 12
    }

    /// Verifica se um movimento dá xeque
    fn gives_check(&self, board: &Board, mv: Move) -> bool {
        // Verifica se há peça na casa de origem antes de fazer o movimento
        if board.get_piece_at(mv.from).is_none() {
            return false;
        }
        
        // Implementação simplificada - faz o movimento e verifica
        let mut temp_board = *board;
        temp_board.make_move(mv);
        temp_board.is_king_in_check(!board.to_move)
    }

    /// Categoriza movimentos em forçantes (capturas, xeques, promoções) e quietos
    fn categorize_moves(&self, moves: &[Move], board: &Board) -> (Vec<Move>, Vec<Move>) {
        let mut forcing_moves = Vec::new();
        let mut quiet_moves = Vec::new();
        
        for &mv in moves {
            // Verifica se é movimento forçante
            let is_capture = self.is_capture(mv, board);
            let is_promotion = mv.promotion.is_some();
            let gives_check = if board.get_piece_at(mv.from).is_some() {
                self.gives_check(board, mv)
            } else {
                false
            };
            
            if is_capture || is_promotion || gives_check {
                forcing_moves.push(mv);
            } else {
                quiet_moves.push(mv);
            }
        }
        
        (forcing_moves, quiet_moves)
    }
}

// Estruturas de resultado
#[derive(Debug)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: Score,
    pub search_info: Vec<SearchInfo>,
}

#[derive(Debug)]
pub struct SearchInfo {
    pub depth: Depth,
    pub score: Score,
    pub nodes: u64,
    pub time: Duration,
    pub pv: Vec<Move>,
}

#[derive(Debug)]
pub struct SearchStats {
    pub nodes_searched: u64,
    pub time_elapsed: Duration,
    pub tt_hit_rate: f64,
    pub tt_usage: f64,
}