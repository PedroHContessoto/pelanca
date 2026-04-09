// Ficheiro: src/engine/search/negamax.rs
// Descrição: Busca Negamax com Alpha-Beta + ID + TT + Move Ordering + Qsearch + NMP + LMR + PVS.

use crate::core::board::Board;
use crate::core::types::{Color, Move, PieceKind};
use crate::engine::traits::*;
use super::tt::{TranspositionTable, Bound, pack_move, unpack_move};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const MAX_PLY: usize = 128;

/// Valor MVV-LVA para cada tipo de peça.
const MVV_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 20000];
const LVA_VALUES: [i32; 6] = [5, 4, 3, 2, 1, 0];

/// Tabela de reduções LMR pré-computada.
/// lmr_table[depth][move_number] = redução em plies.
fn compute_lmr_table() -> [[u8; 64]; 64] {
    let mut table = [[0u8; 64]; 64];
    for depth in 1..64 {
        for moves in 1..64 {
            let r = (((depth as f64).ln() * (moves as f64).ln()) / 2.0) as u8;
            table[depth][moves] = r;
        }
    }
    table
}

/// Motor de busca principal.
pub struct NegamaxSearcher<E: Evaluator> {
    evaluator: E,
    nodes_searched: u64,
    stop: Arc<AtomicBool>,
    start_time: Instant,
    time_limit_ms: u64,
    nodes_limit: Option<u64>,
    tt: TranspositionTable,
    killer_moves: [[Option<Move>; 2]; MAX_PLY],
    history: [[i32; 64]; 64],
    /// Countermove: melhor resposta para cada lance do oponente [from][to]
    countermove: [[Option<Move>; 64]; 64],
    lmr_table: [[u8; 64]; 64],
    game_history: Vec<u64>,
    rep_stack: Vec<u64>,
}

impl<E: Evaluator> NegamaxSearcher<E> {
    pub fn new(evaluator: E) -> Self {
        NegamaxSearcher {
            evaluator,
            nodes_searched: 0,
            stop: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            time_limit_ms: u64::MAX,
            nodes_limit: None,
            tt: TranspositionTable::new(16),
            killer_moves: [[None; 2]; MAX_PLY],
            history: [[0i32; 64]; 64],
            countermove: [[None; 64]; 64],
            lmr_table: compute_lmr_table(),
            game_history: Vec::new(),
            rep_stack: Vec::new(),
        }
    }

    /// Set position history (zobrist hashes) from UCI game moves for repetition detection.
    pub fn set_position_history(&mut self, hashes: Vec<u64>) {
        self.game_history = hashes;
    }

    pub fn tt_mut(&mut self) -> &mut TranspositionTable {
        &mut self.tt
    }

    #[inline(always)]
    fn should_stop(&self) -> bool {
        if self.nodes_searched & 2047 != 0 {
            return false;
        }
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        if let Some(limit) = self.nodes_limit {
            if self.nodes_searched >= limit {
                return true;
            }
        }
        self.start_time.elapsed().as_millis() as u64 >= self.time_limit_ms
    }

    fn calculate_time(&self, config: &SearchConfig, color: Color) -> u64 {
        if let Some(movetime) = config.movetime {
            return movetime;
        }
        if config.infinite {
            return u64::MAX;
        }
        if config.wtime.is_none() && config.btime.is_none() {
            return u64::MAX;
        }
        let our_time = match color {
            Color::White => config.wtime.unwrap_or(60000),
            Color::Black => config.btime.unwrap_or(60000),
        };
        let our_inc = match color {
            Color::White => config.winc.unwrap_or(0),
            Color::Black => config.binc.unwrap_or(0),
        };
        let moves_left = config.movestogo.unwrap_or(25) as u64;

        // Base: tempo / lances restantes + incremento
        let base = our_time / moves_left + our_inc * 3 / 4;

        // Limites de seguranca
        let max_time = our_time * 3 / 10;  // nunca mais de 30% do tempo total
        let min_time = if our_inc > 0 { our_inc / 2 } else { 50 }; // minimo: metade do incremento

        base.max(min_time).min(max_time)
    }

    #[inline]
    fn score_to_tt(score: Score, ply: u8) -> i16 {
        if score > SCORE_MATE - 100 {
            (score + ply as Score) as i16
        } else if score < -SCORE_MATE + 100 {
            (score - ply as Score) as i16
        } else {
            score as i16
        }
    }

    #[inline]
    fn score_from_tt(score: i16, ply: u8) -> Score {
        let s = score as Score;
        if s > SCORE_MATE - 100 {
            s - ply as Score
        } else if s < -SCORE_MATE + 100 {
            s + ply as Score
        } else {
            s
        }
    }

    #[inline]
    fn piece_index(kind: PieceKind) -> usize {
        match kind {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5,
        }
    }

    #[inline]
    fn score_move(&self, board: &Board, mv: Move, tt_move: Option<Move>, ply: usize, prev_move: Option<Move>) -> i32 {
        // 1. TT move — maximo
        if let Some(tm) = tt_move {
            if mv.from == tm.from && mv.to == tm.to && mv.promotion == tm.promotion {
                return 10_000_000;
            }
        }

        // 2. Capturas — MVV-LVA
        let victim = board.get_piece_at(mv.to);
        if let Some(v) = victim {
            let attacker = board.get_piece_at(mv.from);
            let ai = attacker.map(|a| Self::piece_index(a.kind)).unwrap_or(0);
            return 1_000_000 + MVV_VALUES[Self::piece_index(v.kind)] * 10 + LVA_VALUES[ai];
        }

        if mv.is_en_passant {
            return 1_000_000 + MVV_VALUES[0] * 10 + LVA_VALUES[0];
        }

        // 3. Promoções
        if let Some(promo) = mv.promotion {
            return match promo {
                PieceKind::Queen => 900_000,
                PieceKind::Rook => 500_000,
                _ => 300_000,
            };
        }

        // 4. Killers
        if ply < MAX_PLY {
            if let Some(k1) = self.killer_moves[ply][0] {
                if mv.from == k1.from && mv.to == k1.to {
                    return 800_000;
                }
            }
            if let Some(k2) = self.killer_moves[ply][1] {
                if mv.from == k2.from && mv.to == k2.to {
                    return 700_000;
                }
            }
        }

        // 5. Countermove — bonus se este lance é a resposta histórica ao lance anterior
        if let Some(pm) = prev_move {
            if let Some(cm) = self.countermove[pm.from as usize][pm.to as usize] {
                if mv.from == cm.from && mv.to == cm.to {
                    return 600_000;
                }
            }
        }

        // 6. History
        self.history[mv.from as usize][mv.to as usize]
    }

    #[inline]
    fn is_capture(board: &Board, mv: Move) -> bool {
        board.get_piece_at(mv.to).is_some() || mv.is_en_passant
    }

    #[inline]
    fn update_killers(&mut self, mv: Move, ply: usize) {
        if ply >= MAX_PLY { return; }
        if let Some(k1) = self.killer_moves[ply][0] {
            if k1.from == mv.from && k1.to == mv.to { return; }
        }
        self.killer_moves[ply][1] = self.killer_moves[ply][0];
        self.killer_moves[ply][0] = Some(mv);
    }

    #[inline]
    fn update_history(&mut self, mv: Move, depth: u8) {
        let bonus = (depth as i32) * (depth as i32);
        let entry = &mut self.history[mv.from as usize][mv.to as usize];
        *entry += bonus;
        if *entry > 1_000_000 { *entry = 1_000_000; }
    }

    /// Detecção de repetição: verifica se a posição já apareceu no histórico.
    /// Usa twofold no search (padrão Stockfish) — aceita draw na primeira repetição
    /// para evitar loops. Percorre do mais recente para trás, pulando de 2 em 2
    /// (mesma cor a mover).
    #[inline]
    fn is_repetition(&self, hash: u64) -> bool {
        // Percorrer de trás para frente
        self.rep_stack.iter().rev().any(|&h| h == hash)
    }

    /// Verifica se o lado a mover tem peças maiores (não-peão, não-rei) para NMP.
    #[inline]
    fn has_non_pawn_material(board: &Board) -> bool {
        let our = if board.to_move == Color::White { board.white_pieces } else { board.black_pieces };
        (our & (board.knights | board.bishops | board.rooks | board.queens)) != 0
    }

    /// Quiescence search com limite de profundidade.
    fn quiescence(&mut self, board: &Board, mut alpha: Score, beta: Score, ply: u8) -> Score {
        self.nodes_searched += 1;

        if self.should_stop() { return 0; }

        // Limite de profundidade para qsearch (evita explosao em posicoes taticas)
        if ply >= MAX_PLY as u8 - 10 { return self.evaluator.evaluate(board); }

        let stand_pat = self.evaluator.evaluate(board);
        if stand_pat >= beta { return beta; }
        if stand_pat > alpha { alpha = stand_pat; }

        // Big delta pruning — mais agressivo
        if stand_pat + 900 < alpha { return alpha; }

        let captures = board.generate_capture_moves();
        let mut scored: Vec<(Move, i32)> = captures.iter()
            .map(|&mv| {
                let score = if let Some(v) = board.get_piece_at(mv.to) {
                    let ai = board.get_piece_at(mv.from).map(|a| Self::piece_index(a.kind)).unwrap_or(0);
                    MVV_VALUES[Self::piece_index(v.kind)] * 10 + LVA_VALUES[ai]
                } else if mv.is_en_passant {
                    MVV_VALUES[0] * 10 + LVA_VALUES[0]
                } else { 0 };
                (mv, score)
            })
            .collect();

        for i in 0..scored.len() {
            // Pick best
            let mut best_idx = i;
            for j in (i + 1)..scored.len() {
                if scored[j].1 > scored[best_idx].1 { best_idx = j; }
            }
            scored.swap(i, best_idx);
            let mv = scored[i].0;

            // Delta pruning per-move
            let victim_val = if let Some(v) = board.get_piece_at(mv.to) {
                MVV_VALUES[Self::piece_index(v.kind)]
            } else if mv.is_en_passant { MVV_VALUES[0] } else { 0 };
            if stand_pat + victim_val + 200 < alpha { continue; }

            let mut child = *board;
            if !child.make_move(mv) { continue; }
            self.evaluator.update_eval_state(board, &mut child, mv);

            let score = -self.quiescence(&child, -beta, -alpha, ply + 1);
            if self.should_stop() { return 0; }
            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }

        alpha
    }

    fn clear_move_ordering(&mut self) {
        self.killer_moves = [[None; 2]; MAX_PLY];
        for row in &mut self.history {
            for val in row.iter_mut() { *val /= 2; }
        }
    }

    /// Busca raiz com PVS.
    fn search_root(&mut self, board: &Board, depth: u8, mut alpha: Score, beta: Score) -> SearchResult {
        let original_alpha = alpha;
        let moves = board.generate_all_moves();
        let in_check = board.is_king_in_check(board.to_move);

        // Otimizacao: se so tem 1 lance legal, retornar imediatamente
        {
            let mut legal_count = 0u32;
            let mut single_move = None;
            for &mv in &moves {
                let mut test = *board;
                if test.make_move(mv) {
                    legal_count += 1;
                    single_move = Some(mv);
                    if legal_count > 1 { break; }
                }
            }
            if legal_count == 1 {
                if let Some(mv) = single_move {
                    return SearchResult {
                        best_move: Some(mv),
                        ponder_move: None,
                        score: 0,
                        depth,
                        nodes_searched: self.nodes_searched,
                        pv: vec![mv],
                    };
                }
            }
        }

        let tt_move = self.tt.probe(board.zobrist_hash)
            .and_then(|e| unpack_move(e.best_move));

        let mut scored_moves: Vec<(Move, i32)> = moves.iter()
            .map(|&mv| (mv, self.score_move(board, mv, tt_move, 0, None)))
            .collect();
        scored_moves.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        let mut best_move = None;
        let mut best_score = -SCORE_INF;
        let mut best_pv = Vec::new();
        let mut moves_searched = 0u32;
        let mut has_legal_move = false;

        // Push root position hash for repetition detection
        self.rep_stack.push(board.zobrist_hash);

        for (mv, _) in &scored_moves {
            let mv = *mv;
            let mut child = *board;
            if !child.make_move(mv) { continue; }
            self.evaluator.update_eval_state(board, &mut child, mv);

            has_legal_move = true;
            let mut child_pv = Vec::new();
            let effective_depth = if in_check { depth } else { depth - 1 };

            let score;
            if moves_searched == 0 {
                score = -self.negamax(&child, effective_depth, -beta, -alpha, 1, &mut child_pv, Some(mv));
            } else {
                let mut zw_score = -self.negamax(&child, effective_depth, -alpha - 1, -alpha, 1, &mut child_pv, Some(mv));
                if zw_score > alpha && zw_score < beta {
                    child_pv.clear();
                    zw_score = -self.negamax(&child, effective_depth, -beta, -alpha, 1, &mut child_pv, Some(mv));
                }
                score = zw_score;
            }

            moves_searched += 1;

            if self.should_stop() { break; }

            if score > best_score {
                best_score = score;
                best_move = Some(mv);
                best_pv.clear();
                best_pv.push(mv);
                best_pv.extend_from_slice(&child_pv);
            }
            if score > alpha { alpha = score; }
            if alpha >= beta { break; }
        }

        // Pop root hash
        self.rep_stack.pop();

        // Handle checkmate/stalemate
        if !has_legal_move {
            best_score = if in_check { -SCORE_MATE } else { SCORE_DRAW };
        }

        // TT store with correct bound
        if let Some(bm) = best_move {
            let bound = if best_score >= beta {
                Bound::Lower
            } else if best_score > original_alpha {
                Bound::Exact
            } else {
                Bound::Upper
            };
            self.tt.store(
                board.zobrist_hash, depth,
                Self::score_to_tt(best_score, 0), bound, pack_move(bm),
            );
        }

        let ponder_move = if best_pv.len() > 1 { Some(best_pv[1]) } else { None };

        SearchResult {
            best_move,
            ponder_move,
            score: best_score,
            depth,
            nodes_searched: self.nodes_searched,
            pv: best_pv,
        }
    }

    fn negamax(
        &mut self,
        board: &Board,
        depth: u8,
        mut alpha: Score,
        beta: Score,
        ply: u8,
        pv: &mut Vec<Move>,
        prev_move: Option<Move>,
    ) -> Score {
        self.nodes_searched += 1;

        if self.should_stop() { return 0; }

        if board.is_draw_by_insufficient_material() || board.is_draw_by_50_moves() {
            return SCORE_DRAW;
        }

        // Repetition detection: if this position appeared before, it's a draw
        if ply > 0 && self.is_repetition(board.zobrist_hash) {
            return SCORE_DRAW;
        }

        let in_check = board.is_king_in_check(board.to_move);
        let is_pv = beta - alpha > 1;

        // Check extension: se estamos em xeque, estender busca 1 ply
        let depth = if in_check { depth + 1 } else { depth };

        // === TT PROBE ===
        let original_alpha = alpha;
        let mut tt_move: Option<Move> = None;

        if let Some(entry) = self.tt.probe(board.zobrist_hash) {
            tt_move = unpack_move(entry.best_move);

            if !is_pv && entry.depth >= depth {
                let tt_score = Self::score_from_tt(entry.score, ply);
                match entry.bound_type() {
                    Bound::Exact => return tt_score,
                    Bound::Lower => {
                        if tt_score >= beta { return tt_score; }
                        if tt_score > alpha { alpha = tt_score; }
                    }
                    Bound::Upper => {
                        if tt_score <= alpha { return tt_score; }
                    }
                }
            }
        }

        if depth == 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        // === NULL MOVE PRUNING ===
        // Simples e robusto: R = 2 em depth < 6, R = 3 em depth >= 6
        // Sem verification (complexidade extra causa bugs, o basico funciona)
        if !in_check && !is_pv && depth >= 3 && Self::has_non_pawn_material(board) {
            let r = if depth >= 6 { 3 } else { 2 };
            let mut null_board = *board;
            null_board.make_null_move();
            pv.clear();
            let null_score = -self.negamax(&null_board, depth - 1 - r, -beta, -beta + 1, ply + 1, pv, None);
            pv.clear();
            if null_score >= beta {
                return beta;
            }
        }

        // === FUTILITY PRUNING setup ===
        // So chamar evaluate() se depth <= 3 (economiza ~40% dos evaluate calls)
        let can_futility;
        if !is_pv && !in_check && depth <= 3 {
            let static_eval = self.evaluator.evaluate(board);
            let futility_margin = match depth {
                1 => 200,
                2 => 500,
                3 => 900,
                _ => 0,
            };
            can_futility = futility_margin > 0 && static_eval + futility_margin <= alpha;
        } else {
            can_futility = false;
        }

        let moves = board.generate_all_moves();
        let ply_idx = ply as usize;

        // Score in-place: reutilizar moves Vec, evitar segunda alocacao
        let mut scored_moves: Vec<(Move, i32)> = Vec::with_capacity(moves.len());
        for &mv in &moves {
            scored_moves.push((mv, self.score_move(board, mv, tt_move, ply_idx, prev_move)));
        }
        drop(moves); // liberar imediatamente

        let mut best_score = -SCORE_INF;
        let mut best_move_found: Option<Move> = None;
        let mut has_legal_move = false;
        let mut moves_searched = 0u32;

        // Push current position for repetition detection in children
        self.rep_stack.push(board.zobrist_hash);

        // child_pv reutilizado (1 alloc por chamada negamax, nao por lance)
        let mut child_pv = Vec::new();

        for i in 0..scored_moves.len() {
            // Pick best
            let mut best_idx = i;
            for j in (i + 1)..scored_moves.len() {
                if scored_moves[j].1 > scored_moves[best_idx].1 { best_idx = j; }
            }
            scored_moves.swap(i, best_idx);

            let mv = scored_moves[i].0;
            let mut child = *board;
            if !child.make_move(mv) { continue; }
            self.evaluator.update_eval_state(board, &mut child, mv);

            has_legal_move = true;

            let is_cap = Self::is_capture(board, mv);
            let is_promo = mv.promotion.is_some();
            let gives_check = child.is_king_in_check(child.to_move);

            // Futility pruning: skip quiet moves that can't raise alpha
            if can_futility && moves_searched > 0 && !is_cap && !is_promo && !gives_check {
                moves_searched += 1;
                continue;
            }

            child_pv.clear();
            let score;
            if moves_searched == 0 {
                // Primeiro lance: busca completa
                score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, &mut child_pv, Some(mv));
            } else {
                // === LMR: Late Move Reductions ===
                let mut reduction = 0u8;
                if depth >= 3 && moves_searched >= 4 && !is_cap && !is_promo && !in_check && !gives_check {
                    let d = (depth as usize).min(63);
                    let m = (moves_searched as usize).min(63);
                    reduction = self.lmr_table[d][m];
                    // Limitar: pelo menos depth 1
                    if reduction >= depth - 1 {
                        reduction = depth - 2;
                    }
                }

                // PVS: zero-window com possível redução
                let reduced_depth = depth - 1 - reduction;
                let mut zw_score = -self.negamax(&child, reduced_depth, -alpha - 1, -alpha, ply + 1, &mut child_pv, Some(mv));

                // Se reduzido e surpreendeu, re-buscar sem redução
                if reduction > 0 && zw_score > alpha {
                    child_pv.clear();
                    zw_score = -self.negamax(&child, depth - 1, -alpha - 1, -alpha, ply + 1, &mut child_pv, Some(mv));
                }

                // Se PVS surpreendeu, re-buscar com janela completa
                if zw_score > alpha && zw_score < beta {
                    child_pv.clear();
                    zw_score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, &mut child_pv, Some(mv));
                }

                score = zw_score;
            }

            moves_searched += 1;

            if self.should_stop() { return 0; }

            if score > best_score {
                best_score = score;
                best_move_found = Some(mv);
                pv.clear();
                pv.push(mv);
                pv.extend_from_slice(&child_pv);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                if !is_cap && !is_promo {
                    self.update_killers(mv, ply_idx);
                    self.update_history(mv, depth);
                    // Countermove: registar este lance como resposta ao lance anterior
                    if let Some(pm) = prev_move {
                        self.countermove[pm.from as usize][pm.to as usize] = Some(mv);
                    }
                }
                break;
            }
        }

        // Pop current position from repetition stack
        self.rep_stack.pop();

        if !has_legal_move {
            if in_check {
                return -SCORE_MATE + ply as Score;
            } else {
                return SCORE_DRAW;
            }
        }

        // === TT STORE ===
        let bound = if best_score >= beta {
            Bound::Lower
        } else if best_score > original_alpha {
            Bound::Exact
        } else {
            Bound::Upper
        };
        let packed = best_move_found.map(|m| pack_move(m)).unwrap_or(0);
        self.tt.store(
            board.zobrist_hash, depth,
            Self::score_to_tt(best_score, ply), bound, packed,
        );

        best_score
    }
}

impl<E: Evaluator> Searcher for NegamaxSearcher<E> {
    fn search(&mut self, board: &Board, config: &SearchConfig) -> SearchResult {
        let mut noop = |_: &SearchInfo| {};
        self.search_with_info(board, config, &mut noop)
    }

    fn search_with_info(
        &mut self,
        board: &Board,
        config: &SearchConfig,
        info_cb: &mut dyn FnMut(&SearchInfo),
    ) -> SearchResult {
        self.nodes_searched = 0;
        self.start_time = Instant::now();
        self.stop.store(false, Ordering::Relaxed);
        self.time_limit_ms = self.calculate_time(config, board.to_move);
        self.nodes_limit = config.nodes_limit;
        self.tt.new_search();
        self.clear_move_ordering();
        // Initialize repetition stack from game history
        self.rep_stack = self.game_history.clone();

        // Inicializar acumuladores NNUE na posição raiz
        {
            let mut root = *board;
            self.evaluator.init_eval_state(&mut root);
        }

        let mut best_result = SearchResult {
            best_move: None,
            ponder_move: None,
            score: 0,
            depth: 0,
            nodes_searched: 0,
            pv: Vec::new(),
        };

        let max_depth = config.max_depth.min(64);
        let mut prev_score: Score = 0;

        for depth in 1..=max_depth {
            let result;

            if depth <= 4 {
                result = self.search_root(board, depth, -SCORE_INF, SCORE_INF);
            } else {
                // Aspiration windows — simples e robusto
                let mut delta: Score = 50;
                let mut asp_alpha = prev_score - delta;
                let mut asp_beta = prev_score + delta;

                loop {
                    let r = self.search_root(board, depth, asp_alpha, asp_beta);
                    if self.should_stop() {
                        result = r;
                        break;
                    }
                    if r.score <= asp_alpha {
                        asp_alpha = (prev_score - delta).max(-SCORE_INF);
                        delta *= 2;
                    } else if r.score >= asp_beta {
                        asp_beta = (prev_score + delta).min(SCORE_INF);
                        delta *= 2;
                    } else {
                        result = r;
                        break;
                    }
                    if delta > 800 {
                        result = self.search_root(board, depth, -SCORE_INF, SCORE_INF);
                        break;
                    }
                }
            }

            if self.should_stop() && depth > 1 {
                break;
            }

            prev_score = result.score;

            let elapsed = self.start_time.elapsed().as_millis() as u64;
            let nps = if elapsed > 0 {
                result.nodes_searched * 1000 / elapsed
            } else {
                result.nodes_searched * 1000
            };

            let (is_mate, mate_in) = if result.score.abs() > SCORE_MATE - 100 {
                let mate_dist = ((SCORE_MATE - result.score.abs()) + 1) / 2;
                let sign = if result.score > 0 { 1 } else { -1 };
                (true, Some(sign * mate_dist))
            } else {
                (false, None)
            };

            info_cb(&SearchInfo {
                depth,
                score: result.score,
                is_mate,
                mate_in,
                nodes: result.nodes_searched,
                time_ms: elapsed,
                nps,
                hashfull: self.tt.hashfull(),
                pv: result.pv.clone(),
                currmove: None,
                currmovenumber: None,
            });

            best_result = result;

            if self.should_stop() {
                break;
            }

            // Nao comecar nova depth se ja usamos mais de 50% do tempo
            let elapsed_check = self.start_time.elapsed().as_millis() as u64;
            if elapsed_check > self.time_limit_ms / 2 && depth >= 6 {
                break;
            }
        }

        best_result
    }

    fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    fn name(&self) -> &str {
        "negamax-full"
    }
}
