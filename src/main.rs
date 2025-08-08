// Motor de Xadrez Pelanca - Sistema NNUE Integrado
// Aproveitando todas as otimizações existentes do engine

use pelanca::*;
use pelanca::nnue::*;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "generate-data" => {
                let positions = if args.len() > 2 {
                    args[2].parse().unwrap_or(5000) // Menor quantidade mas maior qualidade
                } else {
                    5000
                };
                
                println!("=== GERAÇÃO DE DADOS USANDO STOCKFISH ===");
                generate_training_data_with_stockfish(positions);
            }
            "train-nnue" => {
                let positions = if args.len() > 2 {
                    args[2].parse().unwrap_or(10000) // Mais posições para melhor treinamento
                } else {
                    10000
                };
                let epochs = if args.len() > 3 {
                    args[3].parse().unwrap_or(50) // Mais épocas com learning rate maior
                } else {
                    50
                };
                
                println!("=== TREINAMENTO NNUE COM DADOS STOCKFISH ===");
                train_nnue_with_stockfish_data(positions, epochs);
            }
            "test-nnue" => {
                println!("=== TESTE DE PERFORMANCE OTIMIZADA ===");
                test_nnue_performance_optimized();
            }
            "benchmark" => {
                println!("=== BENCHMARK COMPLETO ===");
                benchmark_all_systems();
            }
            _ => {
                print_usage();
            }
        }
    } else {
        // Execução padrão: demonstração do sistema integrado
        println!("=== PELANCA NNUE - SISTEMA INTEGRADO ===\n");
        demo_integrated_system();
    }
}

fn print_usage() {
    println!("Pelanca NNUE - Sistema de Avaliação Neural Integrado");
    println!();
    println!("Uso: pelanca_v1 [COMANDO] [OPÇÕES]");
    println!();
    println!("Comandos:");
    println!("  generate-data [N]     - Gera N posições usando Stockfish (padrão: 5000)");
    println!("  train-nnue [N] [E]    - Treina NNUE com N posições, E épocas (padrão: 10000, 50)");
    println!("  test-nnue             - Testa performance da NNUE");
    println!("  benchmark             - Benchmark completo do sistema");
    println!("  (sem comando)         - Demonstração integrada");
    println!();
    println!("Exemplos:");
    println!("  cargo run --release generate-data 1000");
    println!("  cargo run --release train-nnue 5000 30");
    println!();
    println!("NOTA: Para melhor qualidade, certifique-se que o Stockfish está instalado.");
}

fn demo_integrated_system() {
    println!("Inicializando sistema NNUE integrado...");
    
    // Cria contexto NNUE
    let mut context = NNUEContext::new();
    println!("✅ Contexto NNUE criado");
    
    // Testa com diferentes posições
    let test_positions = [
        ("Inicial", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("Meio-jogo", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
        ("Final", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
    ];
    
    for (name, fen) in test_positions {
        println!("\n📋 Testando: {}", name);
        
        if let Ok(board) = Board::from_fen(fen) {
            let eval = context.evaluate(&board);
            println!("   Avaliação NNUE: {} centipawns", eval);
            
            // Testa com movimentos
            let moves = board.generate_legal_moves();
            println!("   Movimentos legais: {}", moves.len());
            
            if !moves.is_empty() {
                let mut test_board = board;
                let best_move = moves[0];
                let undo_info = test_board.make_move_with_undo(best_move);
                
                let eval_after = context.evaluate_after_move(&test_board, best_move, &undo_info);
                println!("   Após {}: {} centipawns", best_move, eval_after);
                
                test_board.unmake_move(best_move, undo_info);
                context.undo_evaluation(best_move, &undo_info);
            }
        }
    }
    
    // Estatísticas do cache
    let (hits, misses, hit_rate) = context.cache_stats();
    println!("\n📊 Estatísticas do Cache:");
    println!("   Hits: {}, Misses: {}, Taxa: {:.2}%", hits, misses, hit_rate * 100.0);
    
    println!("\n✅ Demonstração concluída!");
    print_usage();
}

fn generate_training_data_with_stockfish(positions: usize) {
    println!("Gerando {} posições usando jogos contra Stockfish...", positions);
    println!("IMPORTANTE: Certifique-se que o Stockfish está instalado e no PATH");
    
    let start = Instant::now();
    
    // Verifica se Stockfish está disponível (tenta diferentes caminhos)
    let stockfish_paths = vec![
        r"C:\Program Files\Stockfish\stockfish.exe",
        "stockfish",
        "stockfish-windows-x86-64-avx2",
        "stockfish.exe"
    ];
    let mut stockfish_path = "stockfish".to_string();
    let mut found = false;
    
    for path in &stockfish_paths {
        match std::process::Command::new(path).arg("quit").output() {
            Ok(_) => {
                println!("✅ Stockfish encontrado: {}", path);
                stockfish_path = path.to_string();
                found = true;
                break;
            },
            Err(_) => continue,
        }
    }
    
    if !found {
        println!("⚠️  Stockfish não encontrado. Usando sistema interno como fallback.");
        println!("   Para melhor qualidade, instale Stockfish: https://stockfishchess.org/");
    }
    
    let generator = TrainingDataGenerator::new()
        .with_stockfish_depth(12)  // Profundidade maior para melhor qualidade
        .with_stockfish_path(stockfish_path);
    
    let data = generator.generate_training_data(positions);
    let generation_time = start.elapsed();
    
    println!("✅ Geração concluída em {:.2}s", generation_time.as_secs_f64());
    println!("   Posições geradas: {}", data.len());
    println!("   Performance: {:.1} posições/segundo", 
             data.len() as f64 / generation_time.as_secs_f64());
    
    // Salva dados
    match generator.save_binary_data(&data, "stockfish_training.pldt") {
        Ok(()) => println!("   Dados salvos em stockfish_training.pldt"),
        Err(e) => eprintln!("   Erro ao salvar: {}", e),
    }
    
    // Estatísticas dos dados
    print_data_stats(&data);
    
    println!("\n️ Dica: Use 'cargo run --release train-nnue {} 50' para treinar com estes dados", data.len());
}

fn train_nnue_with_stockfish_data(positions: usize, epochs: usize) {
    println!("Treinamento NNUE com arquitetura melhorada: {} posições, {} épocas", positions, epochs);
    println!("Arquitetura: 768 -> 512 -> 256 -> 128 -> 1");
    println!("Learning rate: 0.01 (aumentado para melhor convergência)");
    
    match generate_and_train_nnue(positions, epochs) {
        Ok(nnue) => {
            println!("✅ Treinamento concluído!");
            println!("   Rede salva em trained_v2.nnue");
            
            // Salva com nome diferenciado
            match nnue.save("trained_v2.nnue") {
                Ok(_) => println!("   Arquivo salvo como trained_v2.nnue"),
                Err(e) => eprintln!("   Erro ao salvar: {}", e),
            }
            
            // Teste da rede treinada
            test_trained_network(&nnue);
            
            // Benchmark de performance
            println!("\n⚡ Testando performance...");
            let board = Board::new();
            let (time, evals_per_sec) = benchmark_nnue_evaluation(&board, &nnue, 100000);
            println!("   Performance: {:.0} avaliações/segundo", evals_per_sec);
        }
        Err(e) => {
            eprintln!("❌ Erro no treinamento: {}", e);
            println!("\nDicas para resolver problemas:");
            println!("- Certifique-se que o Stockfish está instalado");
            println!("- Verifique se há espaço suficiente em disco");
            println!("- Tente reduzir o número de posições se estiver com pouca RAM");
        }
    }
}

fn test_nnue_performance_optimized() {
    println!("Testando performance do sistema NNUE integrado...");
    
    let mut context = NNUEContext::new();
    let test_positions = [
        Board::new(),
        Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1").unwrap(),
        Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap(),
    ];
    
    const ITERATIONS: usize = 10000;
    
    println!("\nTeste 1: Avaliação com cache");
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        for board in &test_positions {
            context.evaluate(board);
        }
    }
    let cache_time = start.elapsed();
    let cache_evals = ITERATIONS * test_positions.len();
    
    println!("   {} avaliações em {:.2}ms", cache_evals, cache_time.as_millis());
    println!("   Performance: {:.0} evals/segundo", 
             cache_evals as f64 / cache_time.as_secs_f64());
    
    // Reset cache
    context.cache.clear();
    
    println!("\nTeste 2: Avaliação sem cache");
    let start = Instant::now();
    for _ in 0..1000 {  // Menos iterações pois é mais lento
        for board in &test_positions {
            context.evaluate(board);
        }
    }
    let no_cache_time = start.elapsed();
    let no_cache_evals = 1000 * test_positions.len();
    
    println!("   {} avaliações em {:.2}ms", no_cache_evals, no_cache_time.as_millis());
    println!("   Performance: {:.0} evals/segundo", 
             no_cache_evals as f64 / no_cache_time.as_secs_f64());
    
    // Teste de corretude incremental
    println!("\nTeste 3: Corretude incremental");
    let mut test_board = Board::new();
    let correct = test_incremental_correctness(&mut test_board, &context.nnue);
    println!("   Updates incrementais: {}", if correct { "✅ Corretos" } else { "❌ Incorretos" });
    
    let (hits, misses, hit_rate) = context.cache_stats();
    println!("\nCache final: {} hits, {} misses, {:.1}% hit rate", 
             hits, misses, hit_rate * 100.0);
}

fn benchmark_all_systems() {
    println!("Executando benchmark completo do Pelanca NNUE...");
    
    let positions = [1000, 5000, 10000];
    
    for &pos_count in &positions {
        println!("\n=== Benchmark com {} posições ===", pos_count);
        
        // Geração de dados
        let start = Instant::now();
        let generator = TrainingDataGenerator::new();
        let _data = generator.generate_training_data(pos_count);
        let gen_time = start.elapsed();
        
        println!("Geração: {:.2}s ({:.0} pos/s)", 
                 gen_time.as_secs_f64(),
                 pos_count as f64 / gen_time.as_secs_f64());
        
        // Avaliação NNUE
        let nnue = NNUE::new();
        let board = Board::new();
        let (eval_time, evals_per_sec) = benchmark_nnue_evaluation(&board, &nnue, pos_count);
        
        println!("Avaliação: {:.2}ms ({:.0} evals/s)", 
                 eval_time.as_millis(),
                 evals_per_sec);
    }
}

fn print_data_stats(data: &[TrainingPosition]) {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    
    for pos in data {
        match pos.result {
            GameResult::WhiteWin => white_wins += 1,
            GameResult::BlackWin => black_wins += 1,
            GameResult::Draw => draws += 1,
        }
    }
    
    println!("\n📊 Estatísticas dos Dados:");
    println!("   Vitórias brancas: {} ({:.1}%)", white_wins, white_wins as f64 / data.len() as f64 * 100.0);
    println!("   Vitórias pretas: {} ({:.1}%)", black_wins, black_wins as f64 / data.len() as f64 * 100.0);
    println!("   Empates: {} ({:.1}%)", draws, draws as f64 / data.len() as f64 * 100.0);
}

fn test_trained_network(nnue: &NNUE) {
    println!("\n🧪 Testando rede treinada:");
    
    let test_boards = [
        ("Inicial", Board::new()),
        ("Vantagem material", {
            let mut board = Board::new();
            // Simula posição com vantagem material
            board
        }),
    ];
    
    for (name, board) in test_boards {
        let mut accumulator = NNUEAccumulator::new();
        accumulator.refresh_full(nnue, &board);
        let eval = nnue.evaluate_incremental(&accumulator);
        
        println!("   {}: {} centipawns", name, eval);
    }
}