fn main() {
    println!("Hello, world!");
}
// Ficheiro: src/main.rs
// Descrição: Ponto de entrada com o loop principal do protocolo UCI.

use motor_xadrez::{Board, Move};
use motor_xadrez::evaluation;
use motor_xadrez::search;
use motor_xadrez::transposition::TranspositionTable;
use motor_xadrez::opening_book::{OpeningBook, is_in_opening_phase};
use motor_xadrez::profiling::PROFILER;
use std::io;
use std::time::Instant;

// Estrutura para gerenciar tempos de forma inteligente
#[derive(Debug)]
struct TimeManager {
    wtime: Option<u64>,    // Tempo restante das brancas em ms
    btime: Option<u64>,    // Tempo restante das pretas em ms
    winc: Option<u64>,     // Incremento por lance das brancas em ms
    binc: Option<u64>,     // Incremento por lance das pretas em ms
    movetime: Option<u64>, // Tempo fixo por lance em ms
    depth: Option<u8>,     // Profundidade fixa
    nodes: Option<u64>,    // Número máximo de nós
    infinite: bool,        // Busca infinita
}

#[derive(Debug, Default)]
struct PositionComplexity {
    is_tactical: bool,           // Posição tem características táticas
    is_critical: bool,           // Posição crítica que precisa de muito tempo
    in_check: bool,              // Estamos em xeque
    has_hanging_pieces: bool,    // Temos peças penduradas
    attacked_pieces_count: u32,  // Número de nossas peças atacadas
    piece_density: f32,          // Densidade de peças no tabuleiro (0.0-1.0)
    king_proximity: u8,          // Proximidade dos reis (0-8, maior = mais próximos)
}

impl TimeManager {
    fn new() -> Self {
        TimeManager {
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movetime: None,
            depth: None,
            nodes: None,
            infinite: false,
        }
    }

    // Calcula o tempo ótimo para este lance baseado na situação
    fn calculate_time_for_move(&self, board: &Board, moves_played: u16) -> u64 {
        if let Some(movetime) = self.movetime {
            return movetime;
        }

        if self.infinite {
            return u64::MAX;
        }

        let (my_time, my_inc) = if board.to_move == motor_xadrez::types::Color::White {
            (self.wtime, self.winc)
        } else {
            (self.btime, self.binc)
        };

        if let Some(time_left) = my_time {
            // Detecta características táticas da posição
            let tactical_factors = self.analyze_position_complexity(board);

            // Gestão equilibrada do tempo baseada na fase do jogo
            let mut base_divisor = if moves_played < 10 {
                30  // Abertura: tempo moderado
            } else if moves_played < 25 {
                20  // Meio-jogo: mais tempo para decisões críticas
            } else if moves_played < 50 {
                25  // Final médio: balanceado
            } else {
                18  // Final técnico: mais tempo para cálculo preciso
            };

            // Ajustes inteligentes baseados na complexidade
            if tactical_factors.is_tactical {
                base_divisor = (base_divisor as f32 * 0.65) as u64; // 35% mais tempo em posições táticas (antes era 25%)

                // Ajustes graduais para situações específicas
                if tactical_factors.has_hanging_pieces {
                    base_divisor = base_divisor.saturating_sub(3); // Mais tempo para salvar peças (antes era 2)
                }
                if tactical_factors.in_check {
                    base_divisor = base_divisor.saturating_sub(4); // Muito mais tempo em xeque (antes era 3)
                }
                if tactical_factors.is_critical {
                    base_divisor = base_divisor.saturating_sub(5); // Tempo extra para posições críticas (antes era 2)
                }
            }

            // Não deixar o divisor ficar muito baixo
            base_divisor = base_divisor.max(10); // Reduzido de 15 -> 10 para mais tempo

            let base_time = time_left / base_divisor;
            let increment_bonus = my_inc.unwrap_or(0).saturating_mul(2) / 3;
            let mut time_with_increment = base_time + increment_bonus;

            // Bônus adicional para posições críticas (reduzido)
            if tactical_factors.is_critical {
                time_with_increment = (time_with_increment as f32 * 1.2) as u64; // Era 1.4
            }

            // Limites mais conservadores
            let min_time = if time_left > 10000 { 300 } else { 150 }; // Era 500/200
            let max_time = if tactical_factors.is_tactical {
                time_left / 4 // Era 1/3, agora 1/4 (mais conservador)
            } else {
                time_left / 3 // Era 1/2, agora 1/3
            };

            time_with_increment.max(min_time).min(max_time)
        } else {
            5000 // Era 8000, agora 5000
        }
    }

    /// Analisa a complexidade da posição para determinar gestão de tempo
    fn analyze_position_complexity(&self, board: &Board) -> PositionComplexity {
        let mut complexity = PositionComplexity::default();

        // 1. Verifica se estamos em xeque
        complexity.in_check = board.is_king_in_check(board.to_move);

        // 2. Conta peças atacadas e atacantes
        let our_pieces = if board.to_move == motor_xadrez::types::Color::White {
            board.white_pieces
        } else {
            board.black_pieces
        };
        let enemy_pieces = if board.to_move == motor_xadrez::types::Color::White {
            board.black_pieces
        } else {
            board.white_pieces
        };

        // 3. Detecta peças penduradas (atacadas e não defendidas)
        let our_valuables = (board.knights | board.bishops | board.rooks | board.queens) & our_pieces;
        let mut hanging_pieces = 0;
        let mut attacked_pieces = 0;

        let mut bb = our_valuables;
        while bb != 0 {
            let sq = bb.trailing_zeros() as u8;
            bb &= bb - 1;

            if board.is_square_attacked_by(sq, !board.to_move) {
                attacked_pieces += 1;
                if !board.is_square_attacked_by(sq, board.to_move) {
                    hanging_pieces += 1;
                }
            }
        }

        complexity.has_hanging_pieces = hanging_pieces > 0;
        complexity.attacked_pieces_count = attacked_pieces;

        // 4. Verifica densidade de peças (posições congestionadas são mais táticas)
        let total_pieces = (board.white_pieces | board.black_pieces).count_ones();
        complexity.piece_density = total_pieces as f32 / 64.0;

        // 5. Verifica proximidade dos reis (finais de rei)
        let white_king_bb = board.kings & board.white_pieces;
        let black_king_bb = board.kings & board.black_pieces;

        if white_king_bb != 0 && black_king_bb != 0 {
            let white_king_sq = white_king_bb.trailing_zeros() as u8;
            let black_king_sq = black_king_bb.trailing_zeros() as u8;
            let king_distance = self.square_distance(white_king_sq, black_king_sq);
            complexity.king_proximity = 8 - king_distance; // Maior valor = reis mais próximos
        }

        // 6. Determina se posição é tática (mais sensível)
        complexity.is_tactical = complexity.in_check ||
            hanging_pieces > 0 ||
            attacked_pieces > 1 || // Reduzido de 2 -> 1 (mais sensível)
            (total_pieces <= 20 && complexity.king_proximity > 4) || // Finais ativos (antes era 16/5)
            (total_pieces > 28 && attacked_pieces > 0); // Aberturas com peças atacadas

        // 7. Determina se posição é crítica (precisa de muito tempo)
        complexity.is_critical = hanging_pieces > 0 || // Qualquer peça pendurada é crítico
            (complexity.in_check && attacked_pieces > 0) ||
            (total_pieces <= 12 && complexity.king_proximity > 5) || // Finais críticos
            (attacked_pieces > 2); // Múltiplas peças sob ataque

        complexity
    }

    /// Calcula distância entre duas casas do tabuleiro
    fn square_distance(&self, sq1: u8, sq2: u8) -> u8 {
        let file1 = sq1 % 8;
        let rank1 = sq1 / 8;
        let file2 = sq2 % 8;
        let rank2 = sq2 / 8;

        let file_diff = (file1 as i8 - file2 as i8).abs() as u8;
        let rank_diff = (rank1 as i8 - rank2 as i8).abs() as u8;

        file_diff.max(rank_diff)
    }

    fn get_max_depth(&self) -> u8 {
        self.depth.unwrap_or(50) // Profundidade máxima de 50
    }

    /// Calcula profundidade máxima adaptativa baseada na complexidade
    fn get_adaptive_depth(&self, board: &Board) -> u8 {
        if let Some(fixed_depth) = self.depth {
            return fixed_depth;
        }

        let tactical_factors = self.analyze_position_complexity(board);
        let mut max_depth = 70u8; // Aumentado significativamente de 40 -> 70

        // Ajusta profundidade baseada na complexidade tática
        if tactical_factors.is_critical {
            max_depth = 80; // Aumentado de 45 -> 80 para posições críticas
        } else if tactical_factors.is_tactical {
            max_depth = 75; // Aumentado de 42 -> 75 para posições táticas
        }

        // Em finais simples, ainda assim mantém profundidade alta
        if tactical_factors.piece_density < 0.25 && !tactical_factors.is_tactical {
            max_depth = 60; // Aumentado de 35 -> 60 para finais técnicos
        }

        // Ajustes especiais para aberturas conhecidas
        let total_pieces = (board.white_pieces | board.black_pieces).count_ones();
        if total_pieces > 28 {
            max_depth = 65; // Abertura: profundidade moderada mas alta
        }

        max_depth
    }
}

fn main() {
    // Inicializa as dependências do motor
    motor_xadrez::evaluation::pawn_structure::init_pawn_masks();

    // PERFORMANCE CRÍTICA: Inicializa Magic Bitboards (2-3x speedup)
    motor_xadrez::moves::magic_bitboards::init_magic_bitboards();

    // PERFORMANCE CRÍTICA: Reporta suporte de intrinsics (popcount, BMI, etc.)
    motor_xadrez::intrinsics::init_intrinsics();

    let mut board = Board::new();
    let mut tt = TranspositionTable::new(128); // 16 MB
    let opening_book = OpeningBook::new(); // Carrega livro de aberturas
    let mut use_opening_book = true; // Configurável via UCI
    let mut moves_played = 0u16;

    // Loop principal que espera por comandos da GUI
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let commands: Vec<&str> = input.trim().split_whitespace().collect();

        if let Some(&command) = commands.get(0) {
            match command {
                "uci" => {
                    println!("id name MotorXadrez 2.0");
                    println!("id author Pedro Contessoto");

                    // Opções UCI configuráveis
                    println!("option name Hash type spin default 128 min 16 max 2048");
                    println!("option name Threads type spin default 1 min 1 max 1");
                    println!("option name Ponder type check default false");
                    println!("option name MultiPV type spin default 1 min 1 max 5");
                    println!("option name OwnBook type check default true");

                    println!("uciok");
                }
                "isready" => {
                    println!("readyok");
                }
                "position" => {
                    moves_played = handle_position_command(&mut board, &commands, &mut tt);
                }
                "go" => {
                    handle_go_command(&board, &mut tt, &opening_book, use_opening_book, &commands, moves_played);
                }
                "setoption" => {
                    handle_setoption_command(&commands, &mut use_opening_book);
                }
                "stop" => {
                    // Para a busca atual (para implementar futuramente com threading)
                    println!("bestmove (none)");
                }
                "quit" => {
                    break; // Sai do loop e termina o programa
                }
                // Comandos de profiling personalizados
                "profile" => {
                    handle_profile_command(&commands);
                }
                "benchmark" => {
                    handle_benchmark_command(&board);
                }
                "cache" => {
                    handle_cache_command(&commands);
                }
                "debug" => {
                    handle_debug_command(&commands, &tt);
                }
                "status" => {
                    handle_status_command(&tt);
                }
                "reset" => {
                    // Comando para resetar motor manualmente
                    reset_engine_between_games(&mut tt);
                    println!("info string Engine reset completed");
                }
                "ucinewgame" => {
                    // Comando UCI padrão para nova partida
                    reset_engine_between_games(&mut tt);
                    println!("info string New game started");
                }
                _ => {
                    // Ignora comandos desconhecidos
                }
            }
        }
    }
}

/// Limpa todos os caches e memória do motor entre partidas
fn reset_engine_between_games(tt: &mut TranspositionTable) {
    println!("info string MotorXadrez: Resetting engine memory for new game...");

    // 1. Limpa Transposition Table
    tt.reset_between_games();

    // 2. Limpa cache de mobilidade
    motor_xadrez::evaluation::mobility::reset_mobility_cache();

    // 3. Limpa cache de avaliação (se existir)
    motor_xadrez::evaluation::cache::clear_evaluation_cache();
    println!("info string Evaluation cache cleared");

    println!("info string MotorXadrez: Engine memory cleared successfully");
}

/// Processa o comando "position" e retorna o número de lances jogados
fn handle_position_command(board: &mut Board, commands: &[&str], tt: &mut TranspositionTable) -> u16 {
    let mut move_start_index = 0;
    let mut moves_count = 0u16;

    if commands.get(1) == Some(&"startpos") {
        // Só limpa se for realmente uma nova partida (sem muitos lances)
        let is_new_game = if commands.get(2) == Some(&"moves") {
            // Conta quantos lances há após "moves"
            let moves_count = commands.iter().skip(3).count();
            moves_count <= 2 // Considera nova partida se <= 2 lances
        } else {
            true // Sem moves = nova partida
        };

        if is_new_game {
            reset_engine_between_games(tt);
        }

        *board = Board::new();
        move_start_index = 2;
    } else if commands.get(1) == Some(&"fen") {
        // Encontra o início da string FEN
        let fen_parts: Vec<&str> = commands.iter().skip(2).take_while(|&&c| c != "moves").cloned().collect();
        let fen = fen_parts.join(" ");
        if let Ok(new_board) = Board::from_fen(&fen) {
            *board = new_board;
            // Extrai o número de lances do FEN se possível
            if let Some(fullmove_str) = fen_parts.last() {
                if let Ok(fullmove) = fullmove_str.parse::<u16>() {
                    moves_count = (fullmove - 1) * 2;
                    if board.to_move == motor_xadrez::types::Color::Black {
                        moves_count += 1;
                    }
                }
            }
        }
        move_start_index = 2 + fen_parts.len();
    }

    // Se houver lances após a posição, aplica-os
    if commands.get(move_start_index) == Some(&"moves") {
        for move_str in commands.iter().skip(move_start_index + 1) {
            if let Some(mv) = parse_move(board, move_str) {
                board.make_move(mv);
                moves_count += 1;
            }
        }
    }

    moves_count
}

/// Processa o comando "go" com gestão inteligente de tempo e livro de aberturas
fn handle_go_command(board: &Board, tt: &mut TranspositionTable, opening_book: &OpeningBook, use_book: bool, commands: &[&str], moves_played: u16) {
    let mut time_manager = TimeManager::new();

    // Processa os parâmetros do comando go
    let mut i = 1;
    while i < commands.len() {
        match commands[i] {
            "wtime" => {
                if i + 1 < commands.len() {
                    time_manager.wtime = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "btime" => {
                if i + 1 < commands.len() {
                    time_manager.btime = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "winc" => {
                if i + 1 < commands.len() {
                    time_manager.winc = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "binc" => {
                if i + 1 < commands.len() {
                    time_manager.binc = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "movetime" => {
                if i + 1 < commands.len() {
                    time_manager.movetime = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "depth" => {
                if i + 1 < commands.len() {
                    time_manager.depth = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "nodes" => {
                if i + 1 < commands.len() {
                    time_manager.nodes = commands[i + 1].parse().ok();
                    i += 1;
                }
            },
            "infinite" => {
                time_manager.infinite = true;
            },
            _ => {}
        }
        i += 1;
    }

    // 📚 VERIFICA PRIMEIRO O LIVRO DE ABERTURAS (TEMPORARIAMENTE DESABILITADO)
    if use_book && is_in_opening_phase(board) {
        if let Some((book_move, opening_name)) = opening_book.get_move(board) {
            // Usa movimento do livro de aberturas
            println!("info string Opening book: {}", opening_name);
            println!("bestmove {}", book_move);
            use std::io::{self, Write};
            io::stdout().flush().ok();
            return;
        } else {
            println!("info string Out of opening book");
        }
    }

    // Se não estiver no livro, calcula normalmente
    let time_for_move = time_manager.calculate_time_for_move(board, moves_played);
    let max_depth = time_manager.get_adaptive_depth(board);

    // Força flush para Arena ver imediatamente
    use std::io::{self, Write};

    if let Some((best_move, _final_score)) = search::find_best_move_with_time(board, max_depth, time_for_move, tt) {
        println!("bestmove {}", best_move);
        io::stdout().flush().ok();
    } else {
        // Fallback se não encontrar nenhuma jogada
        let legal_moves = board.generate_legal_moves();
        if let Some(fallback_move) = legal_moves.first() {
            println!("bestmove {}", fallback_move);
        } else {
            println!("bestmove (none)");
        }
        io::stdout().flush().ok();
    }
}

/// Processa o comando "setoption"
fn handle_setoption_command(commands: &[&str], use_opening_book: &mut bool) {
    if commands.len() >= 5 && commands[1] == "name" && commands[3] == "value" {
        let option_name = commands[2];
        let option_value = commands[4];

        match option_name {
            "Hash" => {
                if let Ok(size) = option_value.parse::<usize>() {
                    println!("info string Hash table size set to {} MB", size);
                    // TODO: Redimensionar a tabela de transposição
                }
            },
            "Threads" => {
                if let Ok(threads) = option_value.parse::<usize>() {
                    println!("info string Thread count set to {}", threads);
                    // TODO: Implementar busca multi-threaded
                }
            },
            "Ponder" => {
                let ponder_enabled = option_value == "true";
                println!("info string Pondering {}", if ponder_enabled { "enabled" } else { "disabled" });
            },
            "OwnBook" => {
                *use_opening_book = option_value == "true";
                println!("info string Opening book {}", if *use_opening_book { "enabled" } else { "disabled" });
            },
            _ => {
                println!("info string Unknown option: {}", option_name);
            }
        }
    }
}

/// Função auxiliar para converter uma string (ex: "e2e4") num objeto Move
fn parse_move(board: &Board, move_str: &str) -> Option<Move> {
    let legal_moves = board.generate_legal_moves();
    for mv in legal_moves {
        let mv_str = mv.to_string();
        // Tenta match exato primeiro
        if mv_str == *move_str {
            return Some(mv);
        }
        // Tenta sem a notação de captura 'x'
        let clean_move_str = move_str.replace("x", "").replace("+", "").replace("#", "");
        if mv_str == clean_move_str {
            return Some(mv);
        }
    }
    None
}

/// Manipula comandos de profiling personalizados
fn handle_profile_command(commands: &[&str]) {
    if commands.len() < 2 {
        println!("info string Uso: profile [report|clear|save <filename>|enable|disable]");
        return;
    }

    match commands[1] {
        "report" => {
            println!("info string Gerando relatório de performance...");
            let report = PROFILER.generate_report();
            // Imprime o relatório linha por linha como info string
            for line in report.lines() {
                if !line.trim().is_empty() {
                    println!("info string {}", line);
                }
            }
        },
        "clear" => {
            PROFILER.clear();
            println!("info string Estatísticas de profiling limpas");
        },
        "save" => {
            if commands.len() >= 3 {
                let filename = commands[2];
                match PROFILER.save_report(filename) {
                    Ok(()) => println!("info string Relatório salvo em: {}", filename),
                    Err(e) => println!("info string Erro ao salvar relatório: {}", e),
                }
            } else {
                println!("info string Uso: profile save <filename>");
            }
        },
        "enable" => {
            // Note: O profiler não tem método enable/disable público no momento
            println!("info string Profiling sempre ativo no momento");
        },
        "disable" => {
            println!("info string Profiling sempre ativo no momento");
        },
        _ => {
            println!("info string Comando de profiling desconhecido: {}", commands[1]);
        }
    }
}

/// Executa benchmarks para descobrir gargalos
fn handle_benchmark_command(board: &Board) {
    println!("info string Starting performance benchmark...");

    // Limpa estatísticas anteriores
    PROFILER.clear();

    // Benchmark 1: Geração de movimentos
    let iterations = 1000;
    println!("info string Running move generation benchmark ({} iterations)", iterations);
    let start = Instant::now();
    for _ in 0..iterations {
        let _moves = board.generate_legal_moves();
    }
    let move_gen_time = start.elapsed();
    println!("info string   Move generation: {}μs per call",
             move_gen_time.as_micros() / iterations);

    // Benchmark 2: Avaliação de posição
    println!("info string Running position evaluation benchmark ({} iterations)", iterations);
    let start = Instant::now();
    for _ in 0..iterations {
        let _eval = evaluation::evaluate(board);
    }
    let eval_time = start.elapsed();
    println!("info string   Position evaluation: {}μs per call",
             eval_time.as_micros() / iterations);

    // Benchmark 3: Busca rápida
    let mut tt = TranspositionTable::new(64); // 64 MB para benchmark
    println!("info string Running search benchmark (depth 4)");
    let start = Instant::now();
    let _result = search::find_best_move_with_time(board, 4, 1000, &mut tt);
    let search_time = start.elapsed();
    println!("info string   Search depth 4: {}ms", search_time.as_millis());

    println!("info string Benchmark completed successfully");
}

/// Comando para debug e verificar estado do motor
fn handle_debug_command(commands: &[&str], tt: &TranspositionTable) {
    if commands.len() < 2 {
        println!("info string Usage: debug [memory|tt|cache|all]");
        return;
    }

    match commands[1] {
        "memory" => {
            println!("info string === MEMORY DEBUG ===");

            // Verifica ocupação da TT
            let sample_size = 1000;
            let occupied = tt.estimate_occupancy_sample(sample_size);
            let occupancy_percent = (occupied * 100 / sample_size);
            println!("info string Hash table occupancy: {}% ({}/{})",
                     occupancy_percent, occupied, sample_size);

            // Verifica cache de mobilidade
            if let Ok(cache) = motor_xadrez::evaluation::mobility::MOBILITY_CACHE.try_lock() {
                println!("info string Mobility cache entries: {}", cache.len());
            } else {
                println!("info string Mobility cache: locked");
            }
        },
        "tt" => {
            println!("info string === TRANSPOSITION TABLE DEBUG ===");
            let sample_size = 2000;
            let occupied = tt.estimate_occupancy_sample(sample_size);
            let occupancy_percent = (occupied * 100 / sample_size);
            println!("info string TT size: {} entries", tt.size);
            println!("info string TT occupancy: {}% ({}/{})",
                     occupancy_percent, occupied, sample_size);
            println!("info string TT memory: ~{} MB",
                     (tt.size * std::mem::size_of::<motor_xadrez::transposition::TTEntry>()) / (1024 * 1024));
        },
        "cache" => {
            println!("info string === CACHE DEBUG ===");

            // Mobility cache
            if let Ok(cache) = motor_xadrez::evaluation::mobility::MOBILITY_CACHE.try_lock() {
                println!("info string Mobility cache: {} entries", cache.len());
                println!("info string Mobility cache capacity: {}", cache.capacity());
            }

            // Evaluation cache
            let eval_stats = motor_xadrez::evaluation::cache::get_cache_stats();
            println!("info string Evaluation cache: {}", eval_stats);
        },
        "all" => {
            handle_debug_command(&["debug", "memory"], tt);
            handle_debug_command(&["debug", "cache"], tt);
        },
        _ => {
            println!("info string Unknown debug command: {}", commands[1]);
        }
    }
}

/// Comando para mostrar status geral do motor
fn handle_status_command(tt: &TranspositionTable) {
    println!("info string === MotorXadrez STATUS ===");

    // TT Status
    let sample_size = 1000;
    let occupied = tt.estimate_occupancy_sample(sample_size);
    let occupancy_percent = (occupied * 100 / sample_size);
    println!("info string Hash table: {}% full", occupancy_percent);

    // Cache Status
    if let Ok(cache) = motor_xadrez::evaluation::mobility::MOBILITY_CACHE.try_lock() {
        if cache.len() == 0 {
            println!("info string Mobility cache: CLEAN");
        } else {
            println!("info string Mobility cache: {} entries", cache.len());
        }
    }

    // Memory status
    println!("info string Engine ready for new game");
}

/// Manipula comandos de cache de avaliação
fn handle_cache_command(commands: &[&str]) {
    if commands.len() < 2 {
        println!("info string Uso: cache [stats|clear]");
        return;
    }

    match commands[1] {
        "stats" => {
            let stats = motor_xadrez::evaluation::cache::get_cache_stats();
            println!("info string Cache statistics: {}", stats);
        },
        "clear" => {
            motor_xadrez::evaluation::cache::clear_evaluation_cache();
            println!("info string Evaluation cache cleared");
        },
        _ => {
            println!("info string Comando de cache desconhecido: {}", commands[1]);
        }
    }
}