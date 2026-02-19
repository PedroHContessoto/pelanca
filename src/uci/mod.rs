// Ficheiro: src/uci/mod.rs
// Descrição: Interface UCI (Universal Chess Interface) para comunicação com GUIs como Arena.

pub mod parser;

use std::io::{self, BufRead};
use std::sync::atomic::Ordering;
use crate::core::board::Board;
use crate::engine::traits::*;
use crate::engine::eval::PstEvaluator;
use crate::engine::search::NegamaxSearcher;

/// Motor UCI que mantém estado e comunica via stdin/stdout.
struct UciEngine {
    board: Board,
    searcher: NegamaxSearcher<PstEvaluator>,
}

impl UciEngine {
    fn new() -> Self {
        UciEngine {
            board: Board::new(),
            searcher: NegamaxSearcher::new(PstEvaluator),
        }
    }

    fn new_game(&mut self) {
        self.board = Board::new();
        self.searcher.tt_mut().clear();
    }

    fn set_option(&mut self, tokens: &[&str]) {
        // setoption name Hash value 128
        if tokens.len() >= 4 && tokens[0].eq_ignore_ascii_case("name") && tokens[1].eq_ignore_ascii_case("Hash") && tokens[2].eq_ignore_ascii_case("value") {
            if let Ok(mb) = tokens[3].parse::<usize>() {
                let mb = mb.clamp(1, 1024);
                self.searcher.tt_mut().resize(mb);
            }
        }
    }

    fn set_position(&mut self, tokens: &[&str]) {
        if let Some(board) = parser::parse_position(tokens) {
            self.board = board;
        }
    }

    fn go(&mut self, tokens: &[&str]) {
        let config = parser::parse_go(tokens);

        // Callback para imprimir info UCI durante busca
        let mut info_cb = |info: &SearchInfo| {
            print_info(info);
        };

        let result = self.searcher.search_with_info(&self.board, &config, &mut info_cb);

        // bestmove
        if let Some(mv) = result.best_move {
            if let Some(ponder) = result.ponder_move {
                println!("bestmove {} ponder {}", mv, ponder);
            } else {
                println!("bestmove {}", mv);
            }
        } else {
            println!("bestmove 0000");
        }
    }

    fn stop(&mut self) {
        self.searcher.stop_flag().store(true, Ordering::Relaxed);
    }
}

/// Formata e imprime uma linha "info" UCI.
fn print_info(info: &SearchInfo) {
    let score_str = if info.is_mate {
        format!("score mate {}", info.mate_in.unwrap_or(0))
    } else {
        format!("score cp {}", info.score)
    };

    let pv_str = info.pv.iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let mut line = format!(
        "info depth {} {} nodes {} nps {} time {} hashfull {}",
        info.depth, score_str, info.nodes, info.nps, info.time_ms, info.hashfull
    );

    if !pv_str.is_empty() {
        line.push_str(&format!(" pv {}", pv_str));
    }

    if let Some(currmove) = info.currmove {
        line.push_str(&format!(" currmove {}", currmove));
    }
    if let Some(num) = info.currmovenumber {
        line.push_str(&format!(" currmovenumber {}", num));
    }

    println!("{}", line);
}

/// Imprime identificação e opções do motor.
fn handle_uci() {
    println!("id name Pelanca Mate v1");
    println!("id author Pedro Contessoto");
    println!("option name Hash type spin default 16 min 1 max 1024");
    println!("uciok");
}

/// Loop principal UCI. Lê stdin, despacha comandos.
pub fn run() {
    let mut engine = UciEngine::new();
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() { continue; }

        match tokens[0] {
            "uci" => handle_uci(),
            "isready" => println!("readyok"),
            "ucinewgame" => engine.new_game(),
            "setoption" => engine.set_option(&tokens[1..]),
            "position" => engine.set_position(&tokens[1..]),
            "go" => engine.go(&tokens[1..]),
            "stop" => engine.stop(),
            "quit" => break,
            _ => {} // UCI spec: ignorar comandos desconhecidos
        }
    }
}
