// Ficheiro: src/uci/parser.rs
// Descrição: Parsing de comandos UCI (position, go, move notation).

use crate::core::board::Board;
use crate::core::types::Move;
use crate::engine::traits::SearchConfig;

/// Converte notação de casa "e2" -> index u8 (0-63).
fn square_from_str(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 { return None; }

    let file = bytes[0].wrapping_sub(b'a');
    let rank = bytes[1].wrapping_sub(b'1');

    if file > 7 || rank > 7 { return None; }
    Some(rank * 8 + file)
}

/// Converte string UCI "e2e4" ou "e7e8q" para Move, comparando com lances legais.
pub fn move_from_uci(board: &Board, uci_str: &str) -> Option<Move> {
    if uci_str.len() < 4 { return None; }

    let from = square_from_str(&uci_str[0..2])?;
    let to = square_from_str(&uci_str[2..4])?;

    let promotion_char = if uci_str.len() >= 5 {
        Some(uci_str.as_bytes()[4])
    } else {
        None
    };

    // Encontrar o lance correspondente entre os lances legais
    let moves = board.generate_all_moves();
    for mv in moves {
        if mv.from != from || mv.to != to { continue; }

        // Verificar promoção
        match (mv.promotion, promotion_char) {
            (Some(promo), Some(ch)) => {
                let promo_char = match promo {
                    crate::core::types::PieceKind::Queen => b'q',
                    crate::core::types::PieceKind::Rook => b'r',
                    crate::core::types::PieceKind::Bishop => b'b',
                    crate::core::types::PieceKind::Knight => b'n',
                    _ => continue,
                };
                if promo_char == ch {
                    // Verificar legalidade
                    if board.is_legal_move(mv) {
                        return Some(mv);
                    }
                }
            }
            (None, None) | (None, Some(_)) => {
                // Lance sem promoção
                if mv.promotion.is_none() && board.is_legal_move(mv) {
                    return Some(mv);
                }
            }
            (Some(_), None) => {
                // Promoção sem especificar peça: default para dama
                if mv.promotion == Some(crate::core::types::PieceKind::Queen) && board.is_legal_move(mv) {
                    return Some(mv);
                }
            }
        }
    }

    None
}

/// Parse do comando "position [startpos|fen ...] [moves ...]"
/// Returns (board, position_history) where position_history contains zobrist hashes
/// of all positions reached during the game (for repetition detection).
pub fn parse_position(tokens: &[&str]) -> Option<(Board, Vec<u64>)> {
    if tokens.is_empty() { return None; }

    let (mut board, rest) = if tokens[0] == "startpos" {
        let moves_idx = tokens.iter().position(|&t| t == "moves");
        let board = Board::new();
        let rest = match moves_idx {
            Some(idx) => &tokens[idx + 1..],
            None => &[],
        };
        (board, rest)
    } else if tokens[0] == "fen" {
        let moves_idx = tokens.iter().position(|&t| t == "moves");
        let fen_end = moves_idx.unwrap_or(tokens.len());
        let fen_str = tokens[1..fen_end].join(" ");
        let board = Board::from_fen(&fen_str).ok()?;
        let rest = match moves_idx {
            Some(idx) => &tokens[idx + 1..],
            None => &[],
        };
        (board, rest)
    } else {
        return None;
    };

    // Collect zobrist hashes for repetition detection
    let mut history = Vec::with_capacity(rest.len() + 1);
    history.push(board.zobrist_hash);

    // Aplicar movimentos
    for move_str in rest {
        if let Some(mv) = move_from_uci(&board, move_str) {
            board.make_move(mv);
            history.push(board.zobrist_hash);
        } else {
            return None;
        }
    }

    Some((board, history))
}

/// Parse do comando "go [wtime X] [btime X] [winc X] [binc X] [depth X] ..."
pub fn parse_go(tokens: &[&str]) -> SearchConfig {
    let mut config = SearchConfig::default();

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "wtime" => {
                if i + 1 < tokens.len() {
                    config.wtime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < tokens.len() {
                    config.btime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "winc" => {
                if i + 1 < tokens.len() {
                    config.winc = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "binc" => {
                if i + 1 < tokens.len() {
                    config.binc = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "movestogo" => {
                if i + 1 < tokens.len() {
                    config.movestogo = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "depth" => {
                if i + 1 < tokens.len() {
                    if let Ok(d) = tokens[i + 1].parse::<u8>() {
                        config.max_depth = d;
                    }
                    i += 1;
                }
            }
            "nodes" => {
                if i + 1 < tokens.len() {
                    config.nodes_limit = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < tokens.len() {
                    config.movetime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "infinite" => {
                config.infinite = true;
            }
            _ => {}
        }
        i += 1;
    }

    config
}
