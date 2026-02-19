// Ficheiro: src/uci/mod.rs
// Descrição: Interface UCI (Universal Chess Interface) para comunicação com GUIs como Arena.

pub mod parser;

use std::io::{self, BufRead};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
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
        if tokens.len() >= 4 && tokens[0].eq_ignore_ascii_case("name") && tokens[1].eq_ignore_ascii_case("Hash") && tokens[2].eq_ignore_ascii_case("value") {
            if let Ok(mb) = tokens[3].parse::<usize>() {
                let mb = mb.clamp(1, 1024);
                self.searcher.tt_mut().resize(mb);
            }
        }
    }

    fn set_position(&mut self, tokens: &[&str]) {
        if let Some((board, history)) = parser::parse_position(tokens) {
            self.board = board;
            self.searcher.set_position_history(history);
        }
    }

    fn go(&mut self, tokens: &[&str]) {
        let config = parser::parse_go(tokens);

        let mut info_cb = |info: &SearchInfo| {
            print_info(info);
        };

        let result = self.searcher.search_with_info(&self.board, &config, &mut info_cb);

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
    println!("id name Pelanca Mate v2");
    println!("id author Pedro Contessoto");
    println!("option name Hash type spin default 16 min 1 max 1024");
    println!("uciok");
}

/// Loop principal UCI com threading para suportar "stop" durante busca.
///
/// Stdin é lido numa thread separada que envia linhas via channel.
/// Quando "stop" é recebido durante busca, a thread de stdin seta o AtomicBool
/// diretamente, interrompendo a busca em no máximo 2048 nós.
pub fn run() {
    let mut engine = UciEngine::new();
    let stop_flag = engine.searcher.stop_flag();

    // Channel: stdin reader -> main loop
    let (tx, rx) = mpsc::channel::<String>();

    // Stdin reader thread — also handles "stop" and "quit" by setting the atomic flag
    let stop_for_stdin = stop_flag.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    // Fast-path: if "stop" or "quit", set flag immediately without waiting
                    // for main thread to process the line from the channel
                    let trimmed = l.trim();
                    if trimmed == "stop" || trimmed == "quit" {
                        stop_for_stdin.store(true, Ordering::Relaxed);
                    }
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    loop {
        let line = match rx.recv() {
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
            "stop" => {
                // Already handled by stdin thread setting stop_flag.
                // This processes the queued "stop" after search finishes — no-op.
            }
            "quit" => break,
            _ => {}
        }
    }
}
