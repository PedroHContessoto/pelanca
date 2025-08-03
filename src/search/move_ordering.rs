// Sistema de ordenação de movimentos para melhorar a eficiência do alpha-beta

use super::*;
use crate::core::*;
use std::sync::Arc;

/// Estrutura para armazenar killer moves
pub struct KillerMoves {
    killers: [[Option<Move>; 2]; 64], // 2 killer moves por ply
}

impl KillerMoves {
    pub fn new() -> Self {
        KillerMoves {
            killers: [[None; 2]; 64],
        }
    }

    pub fn add_killer(&mut self, mv: Move, ply: usize) {
        if ply < 64 {
            // Move o killer anterior para segunda posição
            self.killers[ply][1] = self.killers[ply][0];
            // Adiciona novo killer
            self.killers[ply][0] = Some(mv);
        }
    }

    pub fn is_killer(&self, mv: Move, ply: usize) -> bool {
        if ply < 64 {
            self.killers[ply][0] == Some(mv) || self.killers[ply][1] == Some(mv)
        } else {
            false
        }
    }
}

/// Estrutura para history heuristic
pub struct HistoryTable {
    history: [[i32; 64]; 64], // [from][to]
}

impl HistoryTable {
    pub fn new() -> Self {
        HistoryTable {
            history: [[0; 64]; 64],
        }
    }

    pub fn update(&mut self, mv: Move, depth: u8) {
        let bonus = (depth * depth) as i32;
        self.history[mv.from as usize][mv.to as usize] += bonus;

        // Previne overflow
        if self.history[mv.from as usize][mv.to as usize] > 10000 {
            // Reduz todas as entradas pela metade
            for from in 0..64 {
                for to in 0..64 {
                    self.history[from][to] /= 2;
                }
            }
        }
    }

    pub fn get_score(&self, mv: Move) -> i32 {
        self.history[mv.from as usize][mv.to as usize]
    }
}

/// Ordena movimentos para melhorar a eficiência do alpha-beta
pub fn order_moves(
    board: &Board,
    moves: &mut Vec<Move>,
    tt_move: Option<Move>,
    ply: i32,
    controller: &Arc<SearchController>
) {
    // Calcula scores para cada movimento
    let mut move_scores: Vec<(Move, i32)> = moves.iter().map(|&mv| {
        let score = score_move(board, mv, tt_move, ply);
        (mv, score)
    }).collect();

    // Ordena por score decrescente
    move_scores.sort_by(|a, b| b.1.cmp(&a.1));

    // Atualiza vetor original
    for (i, (mv, _)) in move_scores.into_iter().enumerate() {
        moves[i] = mv;
    }
}

/// Calcula score de um movimento para ordenação
fn score_move(
    board: &Board,
    mv: Move,
    tt_move: Option<Move>,
    _ply: i32,
) -> i32 {
    let mut score = 0;

    // 1. Movimento da transposition table tem prioridade máxima
    if Some(mv) == tt_move {
        return 1_000_000;
    }

    // 2. Capturas ordenadas por MVV-LVA
    if is_capture_move(board, mv) {
        score += mvv_lva_score(board, mv);
    }

    // 3. Promoções
    if let Some(promo) = mv.promotion {
        score += match promo {
            PieceKind::Queen => 90000,
            PieceKind::Rook => 50000,
            PieceKind::Bishop => 30000,
            PieceKind::Knight => 30000,
            _ => 0,
        };
    }

    // 4. Killer moves
    // TODO: Implementar killer moves globalmente

    // 5. History heuristic
    // TODO: Implementar history heuristic globalmente

    // 6. Movimentos que dão xeque
    if gives_check(board, mv) {
        score += 5000;
    }

    // 7. Roque geralmente é bom
    if mv.is_castling {
        score += 3000;
    }

    // 8. Penaliza movimentos para trás (exceto capturas)
    if !is_capture_move(board, mv) {
        if board.to_move == Color::White {
            if mv.to / 8 < mv.from / 8 {
                score -= 10;
            }
        } else {
            if mv.to / 8 > mv.from / 8 {
                score -= 10;
            }
        }
    }

    score
}

/// MVV-LVA (Most Valuable Victim - Least Valuable Attacker)
fn mvv_lva_score(board: &Board, mv: Move) -> i32 {
    let mut score = 100000; // Base para capturas

    // Identifica peça atacante
    let from_bb = 1u64 << mv.from;
    let attacker_value = if (board.pawns & from_bb) != 0 { 100 }
    else if (board.knights & from_bb) != 0 { 320 }
    else if (board.bishops & from_bb) != 0 { 330 }
    else if (board.rooks & from_bb) != 0 { 500 }
    else if (board.queens & from_bb) != 0 { 900 }
    else { 10000 }; // Rei

    // Identifica peça capturada
    let to_bb = 1u64 << mv.to;
    let victim_value = if mv.is_en_passant {
        100 // Peão en passant
    } else if (board.pawns & to_bb) != 0 { 100 }
    else if (board.knights & to_bb) != 0 { 320 }
    else if (board.bishops & to_bb) != 0 { 330 }
    else if (board.rooks & to_bb) != 0 { 500 }
    else if (board.queens & to_bb) != 0 { 900 }
    else { 0 };

    // MVV-LVA: capturar peças valiosas com peças baratas é melhor
    score += victim_value * 10 - attacker_value;

    score
}

/// Verifica se um movimento é captura
fn is_capture_move(board: &Board, mv: Move) -> bool {
    let to_bb = 1u64 << mv.to;
    let enemy_pieces = if board.to_move == Color::White {
        board.black_pieces
    } else {
        board.white_pieces
    };

    (enemy_pieces & to_bb) != 0 || mv.is_en_passant
}

/// Verifica se um movimento dá xeque
fn gives_check(board: &Board, mv: Move) -> bool {
    // Implementação simplificada - idealmente faria uma simulação rápida
    let from_bb = 1u64 << mv.from;

    // Verifica se é um movimento de cavalo que pode dar xeque
    if (board.knights & from_bb) != 0 {
        let enemy_king = board.kings & if board.to_move == Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };

        if enemy_king != 0 {
            let king_sq = enemy_king.trailing_zeros() as u8;
            let knight_attacks = crate::moves::knight::get_knight_attacks(mv.to);
            return (knight_attacks & (1u64 << king_sq)) != 0;
        }
    }

    // TODO: Implementar verificação completa para outras peças
    false
}

/// Ordena apenas capturas para busca de quiescência
pub fn order_captures(board: &Board, moves: &mut Vec<Move>) {
    // Filtra apenas capturas
    moves.retain(|&mv| is_capture_move(board, mv));

    // Ordena por MVV-LVA
    moves.sort_by_key(|&mv| -mvv_lva_score(board, mv));
}

/// SEE (Static Exchange Evaluation) simplificado
pub fn see_capture(board: &Board, mv: Move) -> i32 {
    // Implementação simplificada do SEE
    // Retorna valor estimado da captura considerando possíveis recapturas

    let to_bb = 1u64 << mv.to;
    let from_bb = 1u64 << mv.from;

    // Valor da peça capturada
    let captured_value = if mv.is_en_passant {
        100
    } else if (board.pawns & to_bb) != 0 { 100 }
    else if (board.knights & to_bb) != 0 { 320 }
    else if (board.bishops & to_bb) != 0 { 330 }
    else if (board.rooks & to_bb) != 0 { 500 }
    else if (board.queens & to_bb) != 0 { 900 }
    else { 0 };

    // Valor da peça atacante
    let attacker_value = if (board.pawns & from_bb) != 0 { 100 }
    else if (board.knights & from_bb) != 0 { 320 }
    else if (board.bishops & from_bb) != 0 { 330 }
    else if (board.rooks & from_bb) != 0 { 500 }
    else if (board.queens & from_bb) != 0 { 900 }
    else { 10000 };

    // Estimativa simples: assume que o oponente pode recapturar
    // se a peça atacante vale menos que a capturada
    if attacker_value < captured_value {
        captured_value // Boa troca
    } else {
        captured_value - attacker_value // Pode perder material
    }
}