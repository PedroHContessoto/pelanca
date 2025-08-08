// Sistema de treinamento NNUE integrado com perft do Pelanca
// Usa a infraestrutura existente para gerar dados massivamente

use crate::core::{Board, Move, Color};
use super::{NNUE, NNUEAccumulator};
use rayon::prelude::*;
use rand::prelude::*;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufWriter, BufRead, Write};

/// Posição de treinamento otimizada
#[derive(Debug, Clone)]
pub struct TrainingPosition {
    pub board: Board,           // Board completo (mais eficiente que FEN)
    pub evaluation: f32,        // Avaliação normalizada [-1, 1]
    pub result: GameResult,     // Resultado do jogo
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameResult {
    WhiteWin = 1,
    Draw = 0,
    BlackWin = -1,
}

/// Gerador de dados usando Stockfish para jogos de alta qualidade
pub struct TrainingDataGenerator {
    pub positions_per_game: usize,
    pub max_game_moves: usize,
    pub stockfish_depth: u8,
    pub stockfish_path: String,
    pub stockfish_time_ms: u32,
}

impl TrainingDataGenerator {
    pub fn new() -> Self {
        Self {
            positions_per_game: 30,     // Menos posições por jogo, mas maior qualidade
            max_game_moves: 150,        // Jogos mais focados
            stockfish_depth: 10,        // Profundidade do Stockfish
            stockfish_path: "stockfish".to_string(), // Assume Stockfish no PATH
            stockfish_time_ms: 100,     // 100ms por movimento
        }
    }
    
    pub fn with_stockfish_path(mut self, path: String) -> Self {
        self.stockfish_path = path;
        self
    }
    
    pub fn with_stockfish_depth(mut self, depth: u8) -> Self {
        self.stockfish_depth = depth;
        self
    }
    
    /// Gera dados de treinamento usando paralelização nativa do Pelanca
    pub fn generate_training_data(&self, total_positions: usize) -> Vec<TrainingPosition> {
        let cores = rayon::current_num_threads();
        let games_needed = (total_positions / self.positions_per_game).max(1);
        
        println!("Gerando {} posições em {} jogos usando {} cores", 
                total_positions, games_needed, cores);
        
        // Gera jogos sequencialmente para evitar múltiplas instâncias do Stockfish
        let mut all_positions = Vec::new();
        let mut rng = StdRng::seed_from_u64(12345);
        
        for game_id in 0..games_needed {
            if game_id % 10 == 0 {
                println!("Progresso: jogo {}/{}", game_id, games_needed);
            }
            
            let game_positions = self.play_training_game(&mut rng);
            all_positions.extend(game_positions);
            
            // Para se já temos posições suficientes
            if all_positions.len() >= total_positions {
                break;
            }
        }
        
        // Limita ao número desejado de posições
        all_positions.truncate(total_positions);
        
        println!("Geradas {} posições de treinamento", all_positions.len());
        all_positions
    }
    
    /// Joga um jogo usando Stockfish como oponente para dados de alta qualidade
    fn play_training_game<R: Rng>(&self, rng: &mut R) -> Vec<TrainingPosition> {
        match self.play_against_stockfish(rng) {
            Ok(positions) => positions,
            Err(e) => {
                eprintln!("Erro jogando contra Stockfish: {}. Usando jogo interno.", e);
                self.play_internal_game(rng) // Fallback para o sistema interno
            }
        }
    }
    
    /// Joga contra Stockfish para obter dados de alta qualidade
    fn play_against_stockfish<R: Rng>(&self, rng: &mut R) -> Result<Vec<TrainingPosition>, String> {
        let mut positions = Vec::new();
        let mut board = Board::new();
        let mut move_count = 0;
        
        // Inicia processo do Stockfish
        let mut stockfish = Command::new(&self.stockfish_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Erro ao iniciar Stockfish: {}", e))?;
        
        let stdin = stockfish.stdin.take().ok_or("Erro ao obter stdin do Stockfish")?;
        let stdout = stockfish.stdout.take().ok_or("Erro ao obter stdout do Stockfish")?;
        
        let mut writer = BufWriter::new(stdin);
        let mut reader = BufReader::new(stdout);
        
        // Configura Stockfish
        writer.write_all(b"uci\n").map_err(|e| format!("Erro escrevendo para Stockfish: {}", e))?;
        writer.flush().map_err(|e| format!("Erro no flush: {}", e))?;
        
        // Aguarda resposta "uciok"
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).map_err(|e| format!("Erro lendo do Stockfish: {}", e))?;
            if line.trim() == "uciok" {
                break;
            }
        }
        
        // Define opções
        writer.write_all(format!("setoption name Threads value 1\n").as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.write_all(format!("setoption name Hash value 64\n").as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.write_all(b"isready\n")
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.flush()
            .map_err(|e| format!("Erro no flush: {}", e))?;
        
        // Aguarda "readyok"
        loop {
            line.clear();
            reader.read_line(&mut line)
                .map_err(|e| format!("Erro lendo: {}", e))?;
            if line.trim() == "readyok" {
                break;
            }
        }
        
        // Joga o jogo
        let pelanca_plays_white = rng.gen_bool(0.5);
        
        while move_count < self.max_game_moves && !board.is_game_over() {
            let legal_moves = board.generate_legal_moves();
            if legal_moves.is_empty() {
                break;
            }
            
            // Avalia posição atual com Stockfish
            let evaluation = self.get_stockfish_evaluation(&mut writer, &mut reader, &board)?;
            
            // Adiciona aos dados de treinamento
            positions.push(TrainingPosition {
                board,
                evaluation: evaluation / 100.0, // Stockfish retorna em centipawns
                result: GameResult::Draw, // Atualizado no final
            });
            
            let chosen_move = if (board.to_move == Color::White) == pelanca_plays_white {
                // Pelanca escolhe movimento (com alguma aleatoriedade)
                if rng.gen_bool(0.8) {
                    self.choose_best_move_simple(&board, &legal_moves, rng)
                } else {
                    legal_moves[rng.gen_range(0..legal_moves.len())]
                }
            } else {
                // Stockfish escolhe movimento
                self.get_stockfish_move(&mut writer, &mut reader, &board)?
            };
            
            if !board.make_move(chosen_move) {
                break;
            }
            
            move_count += 1;
        }
        
        // Finaliza Stockfish
        writer.write_all(b"quit\n").ok();
        writer.flush().ok();
        stockfish.wait().ok();
        
        // Determina resultado
        let result = self.determine_game_result(&board);
        for pos in &mut positions {
            pos.result = result;
        }
        
        Ok(positions)
    }
    
    /// Obtém avaliação do Stockfish
    fn get_stockfish_evaluation(
        &self,
        writer: &mut BufWriter<std::process::ChildStdin>,
        reader: &mut BufReader<std::process::ChildStdout>,
        board: &Board
    ) -> Result<f32, String> {
        // Envia posição para Stockfish
        let fen = board.to_fen_string();
        writer.write_all(format!("position fen {}\n", fen).as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.write_all(format!("eval\n").as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.flush()
            .map_err(|e| format!("Erro no flush: {}", e))?;
        
        // Lê resposta
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line)
                .map_err(|e| format!("Erro lendo: {}", e))?;
            let trimmed = line.trim();
            
            // Procura por linha de avaliação
            if trimmed.starts_with("Total evaluation:") {
                // Extrai número da avaliação
                if let Some(eval_str) = trimmed.split_whitespace().nth(2) {
                    return eval_str.parse::<f32>()
                        .map_err(|_| "Erro ao parsear avaliação do Stockfish".to_string());
                }
            }
            
            // Se não encontrou avaliação, usa busca rápida
            if trimmed.is_empty() || trimmed.starts_with("info") {
                break;
            }
        }
        
        // Fallback: busca rápida
        writer.write_all(format!("go depth {}\n", 6).as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.flush()
            .map_err(|e| format!("Erro no flush: {}", e))?;
        
        loop {
            line.clear();
            reader.read_line(&mut line)
                .map_err(|e| format!("Erro lendo: {}", e))?;
            let trimmed = line.trim();
            
            if let Some(score_pos) = trimmed.find("score cp ") {
                let score_str = &trimmed[score_pos + 9..];
                if let Some(score_end) = score_str.find(' ') {
                    let score_str = &score_str[..score_end];
                    if let Ok(score) = score_str.parse::<f32>() {
                        return Ok(score);
                    }
                }
            }
            
            if trimmed.starts_with("bestmove") {
                break;
            }
        }
        
        Ok(0.0) // Fallback
    }
    
    /// Obtém movimento do Stockfish
    fn get_stockfish_move(
        &self,
        writer: &mut BufWriter<std::process::ChildStdin>,
        reader: &mut BufReader<std::process::ChildStdout>,
        board: &Board
    ) -> Result<Move, String> {
        let fen = board.to_fen_string();
        writer.write_all(format!("position fen {}\n", fen).as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.write_all(format!("go movetime {}\n", self.stockfish_time_ms).as_bytes())
            .map_err(|e| format!("Erro escrevendo: {}", e))?;
        writer.flush()
            .map_err(|e| format!("Erro no flush: {}", e))?;
        
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line)
                .map_err(|e| format!("Erro lendo: {}", e))?;
            let trimmed = line.trim();
            
            if let Some(bestmove_pos) = trimmed.find("bestmove ") {
                let move_str = &trimmed[bestmove_pos + 9..];
                let move_str = move_str.split_whitespace().next().unwrap_or("");
                
                // Converte movimento UCI para Move do Pelanca
                if let Ok(mv) = self.parse_uci_move(move_str, board) {
                    return Ok(mv);
                }
            }
        }
    }
    
    /// Converte movimento UCI para formato do Pelanca
    fn parse_uci_move(&self, uci: &str, board: &Board) -> Result<Move, String> {
        if uci.len() < 4 {
            return Err("Movimento UCI inválido".to_string());
        }
        
        let from_str = &uci[0..2];
        let to_str = &uci[2..4];
        
        let from = self.square_from_algebraic(from_str)?;
        let to = self.square_from_algebraic(to_str)?;
        
        // Verifica promoção
        let promotion = if uci.len() > 4 {
            match &uci[4..5] {
                "q" => Some(crate::core::PieceKind::Queen),
                "r" => Some(crate::core::PieceKind::Rook),
                "b" => Some(crate::core::PieceKind::Bishop),
                "n" => Some(crate::core::PieceKind::Knight),
                _ => None,
            }
        } else {
            None
        };
        
        // Encontra movimento correspondente na lista de movimentos legais
        let legal_moves = board.generate_legal_moves();
        for mv in legal_moves {
            if mv.from == from && mv.to == to && mv.promotion == promotion {
                return Ok(mv);
            }
        }
        
        Err("Movimento não encontrado na lista de movimentos legais".to_string())
    }
    
    /// Converte notação algébrica para índice de casa
    fn square_from_algebraic(&self, algebraic: &str) -> Result<u8, String> {
        if algebraic.len() != 2 {
            return Err("Notação algébrica inválida".to_string());
        }
        
        let file = algebraic.chars().nth(0).unwrap();
        let rank = algebraic.chars().nth(1).unwrap();
        
        let file_idx = match file {
            'a' => 0, 'b' => 1, 'c' => 2, 'd' => 3,
            'e' => 4, 'f' => 5, 'g' => 6, 'h' => 7,
            _ => return Err("Coluna inválida".to_string()),
        };
        
        let rank_idx = match rank {
            '1' => 0, '2' => 1, '3' => 2, '4' => 3,
            '5' => 4, '6' => 5, '7' => 6, '8' => 7,
            _ => return Err("Fileira inválida".to_string()),
        };
        
        Ok(rank_idx * 8 + file_idx)
    }
    
    /// Escolhe movimento simples para o Pelanca (quando não é turno do Stockfish)
    fn choose_best_move_simple<R: Rng>(&self, board: &Board, moves: &[Move], rng: &mut R) -> Move {
        // Estratégia simples: prefere capturas, depois movimentos centrais
        let mut scored_moves = Vec::new();
        
        for &mv in moves.iter().take(30) { // Limita para performance
            let mut score = 0;
            
            // Bonifica capturas
            let mut temp_board = *board;
            if temp_board.make_move(mv) {
                // Verifica se há captura (simplificado)
                let captured = if temp_board.white_pieces & (1u64 << mv.to) != 0 || 
                                 temp_board.black_pieces & (1u64 << mv.to) != 0 {
                    Some(())
                } else {
                    None
                };
                if captured.is_some() {
                    score += 100;
                }
                
                // Bonifica movimentos centrais
                let file = mv.to % 8;
                let rank = mv.to / 8;
                if (file >= 2 && file <= 5) && (rank >= 2 && rank <= 5) {
                    score += 10;
                }
            }
            
            scored_moves.push((mv, score));
        }
        
        // Ordena por score e adiciona aleatoriedade
        scored_moves.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Escolhe um dos 3 melhores movimentos
        let top_moves = scored_moves.len().min(3);
        scored_moves[rng.gen_range(0..top_moves)].0
    }
    
    /// Jogo interno como fallback
    fn play_internal_game<R: Rng>(&self, rng: &mut R) -> Vec<TrainingPosition> {
        let mut board = Board::new();
        let mut positions = Vec::new();
        let mut move_count = 0;
        
        while move_count < self.max_game_moves && !board.is_game_over() {
            let legal_moves = board.generate_legal_moves();
            if legal_moves.is_empty() {
                break;
            }
            
            // Avaliação básica
            let evaluation = self.evaluate_position_basic(&board);
            
            positions.push(TrainingPosition {
                board,
                evaluation: evaluation / 200.0,
                result: GameResult::Draw,
            });
            
            let chosen_move = self.choose_best_move_simple(&board, &legal_moves, rng);
            
            if !board.make_move(chosen_move) {
                break;
            }
            
            move_count += 1;
        }
        
        let result = self.determine_game_result(&board);
        for pos in &mut positions {
            pos.result = result;
        }
        
        positions
    }
    
    /// Avaliação básica de material
    fn evaluate_position_basic(&self, board: &Board) -> f32 {
        let mut score = 0.0;
        
        score += board.piece_count(Color::White, crate::core::PieceKind::Pawn) as f32 * 100.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Knight) as f32 * 320.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Bishop) as f32 * 330.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Rook) as f32 * 500.0;
        score += board.piece_count(Color::White, crate::core::PieceKind::Queen) as f32 * 900.0;
        
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Pawn) as f32 * 100.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Knight) as f32 * 320.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Bishop) as f32 * 330.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Rook) as f32 * 500.0;
        score -= board.piece_count(Color::Black, crate::core::PieceKind::Queen) as f32 * 900.0;
        
        if board.to_move == Color::Black {
            score = -score;
        }
        
        score.clamp(-2000.0, 2000.0)
    }
    
    /// Determina resultado usando funções do Board
    fn determine_game_result(&self, board: &Board) -> GameResult {
        if board.is_checkmate() {
            if board.to_move == Color::White {
                GameResult::BlackWin
            } else {
                GameResult::WhiteWin
            }
        } else {
            GameResult::Draw
        }
    }
    
    /// Salva dados em formato binário compacto
    pub fn save_binary_data(&self, positions: &[TrainingPosition], filename: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::{BufWriter, Write};
        
        let mut writer = BufWriter::new(File::create(filename)?);
        
        // Header
        writer.write_all(b"PLDT")?; // Pelanca Training Data
        writer.write_all(&(positions.len() as u32).to_le_bytes())?;
        
        // Dados compactos
        for pos in positions {
            // Zobrist hash como identificador da posição
            writer.write_all(&pos.board.zobrist_hash.to_le_bytes())?;
            
            // Avaliação (f32)
            writer.write_all(&pos.evaluation.to_le_bytes())?;
            
            // Resultado (i8)
            writer.write_all(&[(pos.result as i8) as u8])?;
            
            // Informações mínimas do board
            writer.write_all(&pos.board.to_move.to_le_bytes())?;
        }
        
        println!("Dados salvos em {} ({} posições)", filename, positions.len());
        Ok(())
    }
}

impl Default for TrainingDataGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Trainer simples para NNUE
pub struct NNUETrainer {
    learning_rate: f32,
    batch_size: usize,
}

impl NNUETrainer {
    pub fn new() -> Self {
        Self {
            learning_rate: 0.01,    // Aumentado para melhor convergência
            batch_size: 128,        // Batches menores para mais updates frequentes
        }
    }
    
    /// Treinamento básico usando SGD
    pub fn train(&mut self, nnue: &mut NNUE, data: &[TrainingPosition], epochs: usize) -> Result<(), String> {
        println!("Iniciando treinamento: {} posições, {} épocas", data.len(), epochs);
        
        for epoch in 0..epochs {
            let mut total_loss = 0.0;
            let mut batches = 0;
            
            // Shuffle dados
            let mut indices: Vec<usize> = (0..data.len()).collect();
            indices.shuffle(&mut thread_rng());
            
            for batch_indices in indices.chunks(self.batch_size) {
                let batch_loss = self.train_batch(nnue, data, batch_indices)?;
                total_loss += batch_loss;
                batches += 1;
            }
            
            let avg_loss = total_loss / batches as f32;
            println!("Época {}/{}: Loss = {:.6}", epoch + 1, epochs, avg_loss);
        }
        
        Ok(())
    }
    
    /// Treina um batch
    fn train_batch(&self, nnue: &mut NNUE, data: &[TrainingPosition], indices: &[usize]) -> Result<f32, String> {
        let mut total_loss = 0.0;
        let mut gradients = self.init_gradients(nnue);
        
        // Forward + backward para cada exemplo
        for &idx in indices {
            let pos = &data[idx];
            let mut accumulator = NNUEAccumulator::new();
            accumulator.refresh_full(nnue, &pos.board);
            
            // Forward - Normalização melhorada com nova escala
            let prediction = nnue.evaluate_incremental(&accumulator) as f32 / 100.0; // Nova escala
            let target = (pos.evaluation + pos.result as i8 as f32 * 0.1).clamp(-10.0, 10.0); // Faixa maior
            
            // Loss (MSE)
            let error = prediction - target;
            total_loss += error * error;
            
            // Backward (gradiente simples)
            self.accumulate_gradients(&mut gradients, &accumulator, error);
        }
        
        // Aplica gradientes
        self.apply_gradients(nnue, &gradients, indices.len());
        
        Ok(total_loss / indices.len() as f32)
    }
    
    /// Inicializa gradientes para arquitetura multicamada
    fn init_gradients(&self, nnue: &NNUE) -> (Vec<i32>, Vec<i32>, Vec<i32>, Vec<i16>, Vec<i32>, Vec<i16>, Vec<i32>, i32) {
        (
            vec![0; nnue.feature_weights.len()],     // feature_weights
            vec![0; nnue.feature_bias.len()],        // feature_bias
            vec![0; nnue.hidden1_weights.len()],     // hidden1_weights
            vec![0; nnue.hidden1_bias.len()],        // hidden1_bias
            vec![0; nnue.hidden2_weights.len()],     // hidden2_weights
            vec![0; nnue.hidden2_bias.len()],        // hidden2_bias
            vec![0; nnue.output_weights.len()],      // output_weights
            0                                        // output_bias
        )
    }
    
    /// Acumula gradientes para arquitetura multicamada
    fn accumulate_gradients(&self, gradients: &mut (Vec<i32>, Vec<i32>, Vec<i32>, Vec<i16>, Vec<i32>, Vec<i16>, Vec<i32>, i32), accumulator: &NNUEAccumulator, error: f32) {
        let error_scaled = (error * 1000.0) as i32; // Precisão ajustada
        
        // Gradientes da saída (simplificado para esta versão)
        for i in 0..accumulator.accumulator.len().min(super::HIDDEN_SIZE_3) {
            if accumulator.accumulator[i] > 0 {
                let activation = accumulator.accumulator[i].max(0) / 100;
                gradients.6[i] += (error_scaled * activation) / 1000; // output_weights
            }
        }
        
        // Output bias
        gradients.7 += error_scaled;
        
        // Feature bias (primeira camada)
        for i in 0..accumulator.accumulator.len().min(super::HIDDEN_SIZE_1) {
            if accumulator.accumulator[i] > 0 {
                gradients.1[i] += error_scaled / 500; // feature_bias
            }
        }
        
        // Hidden layer biases (atualização simples)
        for i in 0..gradients.3.len() {
            gradients.3[i] += (error_scaled / 1000) as i16; // hidden1_bias
        }
        
        for i in 0..gradients.5.len() {
            gradients.5[i] += (error_scaled / 1000) as i16; // hidden2_bias
        }
    }
    
    /// Aplica gradientes usando SGD com learning rate adaptativo
    fn apply_gradients(&self, nnue: &mut NNUE, gradients: &(Vec<i32>, Vec<i32>, Vec<i32>, Vec<i16>, Vec<i32>, Vec<i16>, Vec<i32>, i32), batch_size: usize) {
        let lr_scaled = (self.learning_rate * 1000.0 / batch_size as f32) as i32;
        let lr_scaled_small = lr_scaled / 10; // Learning rate menor para camadas ocultas
        
        // Atualiza feature bias
        for i in 0..nnue.feature_bias.len() {
            let update = (gradients.1[i] * lr_scaled) / 1000;
            nnue.feature_bias[i] = (nnue.feature_bias[i] - update).clamp(-10000, 10000);
        }
        
        // Atualiza hidden1 weights
        for i in 0..nnue.hidden1_weights.len() {
            let update = (gradients.2[i] * lr_scaled_small) / 1000;
            nnue.hidden1_weights[i] = (nnue.hidden1_weights[i] as i32 - update).clamp(-127, 127) as i8;
        }
        
        // Atualiza hidden1 bias
        for i in 0..nnue.hidden1_bias.len() {
            let update = (gradients.3[i] as i32 * lr_scaled) / 1000;
            nnue.hidden1_bias[i] = (nnue.hidden1_bias[i] as i32 - update).clamp(-1000, 1000) as i16;
        }
        
        // Atualiza hidden2 weights
        for i in 0..nnue.hidden2_weights.len() {
            let update = (gradients.4[i] * lr_scaled_small) / 1000;
            nnue.hidden2_weights[i] = (nnue.hidden2_weights[i] as i32 - update).clamp(-127, 127) as i8;
        }
        
        // Atualiza hidden2 bias
        for i in 0..nnue.hidden2_bias.len() {
            let update = (gradients.5[i] as i32 * lr_scaled) / 1000;
            nnue.hidden2_bias[i] = (nnue.hidden2_bias[i] as i32 - update).clamp(-1000, 1000) as i16;
        }
        
        // Atualiza output weights
        for i in 0..nnue.output_weights.len() {
            let update = (gradients.6[i] * lr_scaled) / 1000;
            nnue.output_weights[i] = (nnue.output_weights[i] as i32 - update).clamp(-127, 127) as i8;
        }
        
        // Atualiza output bias
        let bias_update = (gradients.7 * lr_scaled) / 1000;
        nnue.output_bias = (nnue.output_bias - bias_update).clamp(-100000, 100000);
    }
}

impl Default for NNUETrainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Função conveniente para gerar e treinar
pub fn generate_and_train_nnue(positions: usize, epochs: usize) -> Result<NNUE, String> {
    // Verifica se Stockfish está disponível (tenta diferentes caminhos)
    let stockfish_paths = vec![
        r"C:\Program Files\Stockfish\stockfish.exe",
        "stockfish",
        "stockfish-windows-x86-64-avx2",
        "stockfish.exe"
    ];
    let mut stockfish_path = "stockfish".to_string();
    
    for path in &stockfish_paths {
        match std::process::Command::new(path).arg("quit").output() {
            Ok(_) => {
                stockfish_path = path.to_string();
                break;
            },
            Err(_) => continue,
        }
    }
    
    // Gera dados
    let generator = TrainingDataGenerator::new()
        .with_stockfish_path(stockfish_path);
    let data = generator.generate_training_data(positions);
    
    // Salva dados para debug
    generator.save_binary_data(&data, "training_data.pldt")
        .map_err(|e| format!("Erro ao salvar dados: {}", e))?;
    
    // Treina rede
    let mut nnue = NNUE::new();
    let mut trainer = NNUETrainer::new();
    trainer.train(&mut nnue, &data, epochs)?;
    
    // Salva rede treinada
    nnue.save("trained.nnue")
        .map_err(|e| format!("Erro ao salvar rede: {}", e))?;
    
    Ok(nnue)
}