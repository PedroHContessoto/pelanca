// Geração de dados de treinamento para NNUE
// Usa o engine existente do Pelanca para gerar posições com avaliações

use crate::core::{Board, Color, Move};
use rayon::prelude::*;
use rand::Rng;

/// Estrutura para uma posição de treinamento
#[derive(Debug, Clone)]
pub struct TrainingPosition {
    pub fen: String,
    pub evaluation: f32,    // Avaliação em centipawns (normalizada)
    pub result: GameResult, // Resultado final do jogo
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameResult {
    WhiteWin,
    BlackWin, 
    Draw,
    Unknown,
}

impl GameResult {
    pub fn to_score(&self) -> f32 {
        match self {
            GameResult::WhiteWin => 1.0,
            GameResult::BlackWin => -1.0,
            GameResult::Draw => 0.0,
            GameResult::Unknown => 0.0,
        }
    }
}

/// Gerador de dados de treinamento
pub struct DataGenerator {
    positions_per_thread: usize,
    max_depth: u8,
    max_moves_per_game: usize,
}

impl DataGenerator {
    pub fn new() -> Self {
        Self {
            positions_per_thread: 10000,
            max_depth: 6,
            max_moves_per_game: 200,
        }
    }
    
    /// Gera dados de treinamento usando self-play paralelo
    pub fn generate_training_data(&self, total_positions: usize) -> Vec<TrainingPosition> {
        let num_threads = rayon::current_num_threads();
        let positions_per_thread = total_positions / num_threads;
        
        println!("Gerando {} posições usando {} threads...", total_positions, num_threads);
        println!("Posições por thread: {}", positions_per_thread);
        
        // Gera dados em paralelo
        (0..num_threads)
            .into_par_iter()
            .map(|thread_id| {
                println!("Thread {} iniciada", thread_id);
                self.generate_positions_thread(positions_per_thread, thread_id as u64)
            })
            .flatten()
            .collect()
    }
    
    /// Gera posições em uma thread específica
    fn generate_positions_thread(&self, count: usize, seed: u64) -> Vec<TrainingPosition> {
        use rand::{SeedableRng, prelude::*};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut positions = Vec::new();
        
        let games_needed = (count / 50).max(1); // ~50 posições por jogo
        
        for game_id in 0..games_needed {
            if game_id % 100 == 0 {
                println!("Thread {}: jogo {}/{}", seed, game_id, games_needed);
            }
            
            let game_positions = self.play_training_game(&mut rng);
            positions.extend(game_positions);
            
            if positions.len() >= count {
                positions.truncate(count);
                break;
            }
        }
        
        println!("Thread {} finalizada com {} posições", seed, positions.len());
        positions
    }
    
    /// Joga um jogo completo para gerar posições de treinamento
    fn play_training_game<R: Rng>(&self, rng: &mut R) -> Vec<TrainingPosition> {
        let mut board = Board::new();
        let mut positions = Vec::new();
        let mut move_count = 0;
        
        while move_count < self.max_moves_per_game {
            // Verifica fim de jogo
            if board.is_game_over() {
                break;
            }
            
            // Gera movimentos legais
            let moves = board.generate_legal_moves();
            if moves.is_empty() {
                break;
            }
            
            // Avalia posição atual com busca simples
            let evaluation = self.evaluate_position(&board);
            
            // Adiciona posição aos dados de treinamento
            positions.push(TrainingPosition {
                fen: self.board_to_fen(&board),
                evaluation: evaluation / 100.0, // Normaliza para [-1, 1] aproximadamente
                result: GameResult::Unknown,    // Será atualizado no final
            });
            
            // Escolhe movimento (mistura de busca + aleatoriedade)
            let chosen_move = if rng.gen_bool(0.8) {
                // 80% das vezes: escolhe melhor movimento da busca
                self.choose_best_move(&board, &moves)
            } else {
                // 20% das vezes: movimento aleatório para diversidade
                moves[rng.gen_range(0..moves.len())]
            };
            
            // Executa movimento
            board.make_move(chosen_move);
            move_count += 1;
        }
        
        // Determina resultado do jogo
        let result = self.determine_game_result(&board);
        
        // Atualiza resultado em todas as posições
        for pos in &mut positions {
            pos.result = result;
        }
        
        positions
    }
    
    /// Avaliação simples baseada em material e posição
    fn evaluate_position(&self, board: &Board) -> f32 {
        // Avaliação básica de material
        let mut score = 0.0;
        
        // Material das brancas
        score += board.piece_count(Color::White, crate::core::PieceKind::Pawn) as f32 * 100.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Knight) as f32 * 320.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Bishop) as f32 * 330.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Rook) as f32 * 500.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Queen) as f32 * 900.0;
        
        // Material das pretas
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Pawn) as f32 * 100.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Knight) as f32 * 320.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Bishop) as f32 * 330.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Rook) as f32 * 500.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Queen) as f32 * 900.0;
        
        // Ajusta pela perspectiva do jogador
        if board.to_move == Color::Black {
            score = -score;
        }
        
        // Adiciona ruído para variabilidade
        score += (rand::random::<f32>() - 0.5) * 20.0;
        
        score
    }
    
    /// Escolhe o melhor movimento usando busca simples
    fn choose_best_move(&self, board: &Board, moves: &[Move]) -> Move {
        let mut best_move = moves[0];
        let mut best_score = f32::NEG_INFINITY;
        
        for &mv in moves {
            let mut temp_board = *board;
            temp_board.make_move(mv);
            
            let score = -self.evaluate_position(&temp_board); // Nega para o oponente
            
            if score > best_score {
                best_score = score;
                best_move = mv;
            }
        }
        
        best_move
    }
    
    /// Determina o resultado final do jogo
    fn determine_game_result(&self, board: &Board) -> GameResult {
        if board.is_checkmate() {
            // O jogador atual está em xeque-mate, então o oponente venceu
            if board.to_move == Color::White {
                GameResult::BlackWin
            } else {
                GameResult::WhiteWin
            }
        } else if board.is_stalemate() || board.is_draw_by_insufficient_material() || board.is_draw_by_50_moves() {
            GameResult::Draw
        } else {
            GameResult::Draw // Jogos que não terminaram são considerados empates
        }
    }
    
    /// Converte board para FEN (simplificado)
    fn board_to_fen(&self, board: &Board) -> String {
        // TODO: Implementar conversão completa para FEN
        // Por enquanto, usa hash como identificador único
        format!("position_{:x}", board.zobrist_hash)
    }
    
    /// Salva dados de treinamento em arquivo
    pub fn save_training_data(&self, positions: &[TrainingPosition], filename: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::{BufWriter, Write};
        
        let file = File::create(filename)?;
        let mut writer = BufWriter::new(file);
        
        // Header
        writeln!(writer, "# Pelanca NNUE Training Data")?;
        writeln!(writer, "# Format: FEN eval result")?;
        writeln!(writer, "# Positions: {}", positions.len())?;
        writeln!(writer)?;
        
        // Dados
        for pos in positions {
            writeln!(writer, "{} {:.6} {}", 
                pos.fen, 
                pos.evaluation,
                match pos.result {
                    GameResult::WhiteWin => "1-0",
                    GameResult::BlackWin => "0-1", 
                    GameResult::Draw => "1/2-1/2",
                    GameResult::Unknown => "*",
                }
            )?;
        }
        
        Ok(())
    }
}

/// Função conveniente para gerar dados
pub fn generate_and_save_data(positions: usize, filename: &str) -> std::io::Result<()> {
    let generator = DataGenerator::new();
    let data = generator.generate_training_data(positions);
    generator.save_training_data(&data, filename)?;
    println!("Salvos {} posições em {}", data.len(), filename);
    Ok(())
}