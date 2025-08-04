// Interface UCI (Universal Chess Interface) para o motor Pelanca

use pelanca::*;
use pelanca::search::*;
use std::io::{self, BufRead};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

struct UCIEngine {
    board: Board,
    search_controller: Option<Arc<SearchController>>,
    search_thread: Option<thread::JoinHandle<()>>,
}

impl UCIEngine {
    fn new() -> Self {
        UCIEngine {
            board: Board::new(),
            search_controller: None,
            search_thread: None,
        }
    }

    fn run(&mut self) {
        let stdin = io::stdin();

        for line in stdin.lock().lines() {
            if let Ok(input) = line {
                let parts: Vec<&str> = input.trim().split_whitespace().collect();

                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "uci" => self.handle_uci(),
                    "isready" => println!("readyok"),
                    "ucinewgame" => self.handle_new_game(),
                    "position" => self.handle_position(&parts),
                    "go" => self.handle_go(&parts),
                    "stop" => self.handle_stop(),
                    "quit" => break,
                    _ => {} // Ignora comandos desconhecidos
                }
            }
        }
    }

    fn handle_uci(&self) {
        println!("id name Pelanca v11.0");
        println!("id author Pedro Contessoto");

        // Opções UCI
        println!("option name Hash type spin default 256 min 1 max 16384");
        println!("option name Threads type spin default {} min 1 max 128", num_cpus::get());
        println!("option name Ponder type check default false");

        println!("uciok");
    }

    fn handle_new_game(&mut self) {
        self.board = Board::new();

        // Limpa TT se existir
        if let Some(ref controller) = self.search_controller {
            controller.tt.clear();
        }
    }

    fn handle_position(&mut self, parts: &[&str]) {
        if parts.len() < 2 {
            return;
        }

        let mut idx = 1;

        match parts[idx] {
            "startpos" => {
                self.board = Board::new();
                idx += 1;
            }
            "fen" => {
                idx += 1;
                let mut fen_parts = Vec::new();

                // Coleta partes do FEN até "moves" ou fim
                while idx < parts.len() && parts[idx] != "moves" {
                    fen_parts.push(parts[idx]);
                    idx += 1;
                }

                let fen = fen_parts.join(" ");
                match Board::from_fen(&fen) {
                    Ok(board) => self.board = board,
                    Err(e) => eprintln!("info string Invalid FEN: {}", e),
                }
            }
            _ => return,
        }

        // Processa movimentos se houver
        if idx < parts.len() && parts[idx] == "moves" {
            idx += 1;

            while idx < parts.len() {
                if let Some(mv) = self.parse_move(parts[idx]) {
                    self.board.make_move(mv);
                }
                idx += 1;
            }
        }
    }

    fn handle_go(&mut self, parts: &[&str]) {
        // Para busca anterior se existir
        self.handle_stop();

        let mut config = SearchConfig::default();
        let mut idx = 1;

        // Parse parâmetros
        while idx < parts.len() {
            match parts[idx] {
                "depth" => {
                    if idx + 1 < parts.len() {
                        if let Ok(d) = parts[idx + 1].parse::<u8>() {
                            config.max_depth = d;
                        }
                        idx += 2;
                    } else {
                        idx += 1;
                    }
                }
                "movetime" => {
                    if idx + 1 < parts.len() {
                        if let Ok(ms) = parts[idx + 1].parse::<u64>() {
                            config.max_time = Some(Duration::from_millis(ms));
                        }
                        idx += 2;
                    } else {
                        idx += 1;
                    }
                }
                "wtime" => {
                    if idx + 1 < parts.len() && self.board.to_move == Color::White {
                        if let Ok(ms) = parts[idx + 1].parse::<u64>() {
                            // Usa 2% do tempo restante
                            config.max_time = Some(Duration::from_millis(ms / 50));
                        }
                        idx += 2;
                    } else {
                        idx += 2;
                    }
                }
                "btime" => {
                    if idx + 1 < parts.len() && self.board.to_move == Color::Black {
                        if let Ok(ms) = parts[idx + 1].parse::<u64>() {
                            // Usa 2% do tempo restante
                            config.max_time = Some(Duration::from_millis(ms / 50));
                        }
                        idx += 2;
                    } else {
                        idx += 2;
                    }
                }
                "infinite" => {
                    config.max_time = None;
                    config.max_depth = 64;
                    idx += 1;
                }
                _ => idx += 1,
            }
        }

        // Inicia busca em thread separada
        let controller = Arc::new(SearchController::new(config));
        self.search_controller = Some(controller.clone());

        let board_clone = self.board.clone();
        let search_thread = thread::spawn(move || {
            let (best_move, _stats) = search(&mut board_clone.clone(), controller);
            println!("bestmove {}", best_move);
        });

        self.search_thread = Some(search_thread);
    }

    fn handle_stop(&mut self) {
        if let Some(ref controller) = self.search_controller {
            controller.stop();
        }

        if let Some(thread) = self.search_thread.take() {
            let _ = thread.join();
        }
    }

    fn parse_move(&self, move_str: &str) -> Option<Move> {
        if move_str.len() < 4 {
            return None;
        }

        let bytes = move_str.as_bytes();
        let from_file = (bytes[0] - b'a') as u8;
        let from_rank = (bytes[1] - b'1') as u8;
        let to_file = (bytes[2] - b'a') as u8;
        let to_rank = (bytes[3] - b'1') as u8;

        if from_file > 7 || from_rank > 7 || to_file > 7 || to_rank > 7 {
            return None;
        }

        let from = from_rank * 8 + from_file;
        let to = to_rank * 8 + to_file;

        // Verifica promoção
        let promotion = if move_str.len() > 4 {
            match bytes[4] {
                b'q' => Some(PieceKind::Queen),
                b'r' => Some(PieceKind::Rook),
                b'b' => Some(PieceKind::Bishop),
                b'n' => Some(PieceKind::Knight),
                _ => None,
            }
        } else {
            None
        };

        // Verifica se é roque
        let is_castling = if (self.board.kings & (1u64 << from)) != 0 {
            let king_start = if self.board.to_move == Color::White { 4 } else { 60 };
            from == king_start && (to == king_start + 2 || to == king_start - 2)
        } else {
            false
        };

        // Verifica en passant
        let is_en_passant = if let Some(ep_target) = self.board.en_passant_target {
            to == ep_target && (self.board.pawns & (1u64 << from)) != 0
        } else {
            false
        };

        Some(Move {
            from,
            to,
            promotion,
            is_castling,
            is_en_passant,
        })
    }
}

fn main() {
    let mut engine = UCIEngine::new();
    engine.run();
}