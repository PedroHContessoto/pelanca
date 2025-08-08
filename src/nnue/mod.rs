// NNUE (Efficiently Updatable Neural Network) integration with Pelanca
// Aproveitando ao máximo as otimizações existentes do engine

pub mod features;
pub mod network;
pub mod evaluation;
pub mod training;

pub use features::*;
pub use evaluation::*;
pub use training::*;

// Configurações da arquitetura NNUE otimizada para Pelanca
pub const FEATURE_SIZE: usize = 768;      // HalfKP features
pub const HIDDEN_SIZE_1: usize = 512;     // Primeira camada oculta - maior capacidade
pub const HIDDEN_SIZE_2: usize = 256;     // Segunda camada oculta - refinamento
pub const HIDDEN_SIZE_3: usize = 128;     // Terceira camada oculta - compressão
pub const OUTPUT_SIZE: usize = 1;

/// Estrutura principal do NNUE com arquitetura melhorada
#[derive(Debug, Clone)]
pub struct NNUE {
    // Primeira camada (features -> hidden1)
    pub feature_weights: Vec<i16>,      // FEATURE_SIZE x HIDDEN_SIZE_1
    pub feature_bias: Vec<i32>,         // HIDDEN_SIZE_1
    
    // Segunda camada (hidden1 -> hidden2)
    pub hidden1_weights: Vec<i8>,       // HIDDEN_SIZE_1 x HIDDEN_SIZE_2
    pub hidden1_bias: Vec<i16>,         // HIDDEN_SIZE_2
    
    // Terceira camada (hidden2 -> hidden3)
    pub hidden2_weights: Vec<i8>,       // HIDDEN_SIZE_2 x HIDDEN_SIZE_3
    pub hidden2_bias: Vec<i16>,         // HIDDEN_SIZE_3
    
    // Camada de saída (hidden3 -> output)
    pub output_weights: Vec<i8>,        // HIDDEN_SIZE_3
    pub output_bias: i32,
    
    /// Fatores de escala para diferentes camadas
    pub feature_scale: i32,
    pub hidden_scale: i32,
    pub output_scale: i32,
}

impl NNUE {
    /// Cria nova rede com pesos aleatórios e inicialização Xavier
    pub fn new() -> Self {
        use rand::prelude::*;
        let mut rng = thread_rng();
        
        // Escalas otimizadas para cada camada
        const FEATURE_SCALE: i32 = 128;    // Maior precisão para features
        const HIDDEN_SCALE: i32 = 64;      // Escala intermediária
        const OUTPUT_SCALE: i32 = 400;     // Escala para centipawns
        
        // Inicialização Xavier para melhor convergência
        let feature_range = ((6.0 / (FEATURE_SIZE + HIDDEN_SIZE_1) as f32).sqrt() * FEATURE_SCALE as f32) as i16;
        let hidden1_range = ((6.0 / (HIDDEN_SIZE_1 + HIDDEN_SIZE_2) as f32).sqrt() * 32.0) as i8;
        let hidden2_range = ((6.0 / (HIDDEN_SIZE_2 + HIDDEN_SIZE_3) as f32).sqrt() * 32.0) as i8;
        let output_range = ((6.0 / (HIDDEN_SIZE_3 + 1) as f32).sqrt() * 32.0) as i8;
        
        Self {
            // Primeira camada
            feature_weights: (0..FEATURE_SIZE * HIDDEN_SIZE_1)
                .map(|_| rng.gen_range(-feature_range..=feature_range))
                .collect(),
            feature_bias: vec![0; HIDDEN_SIZE_1],
            
            // Segunda camada
            hidden1_weights: (0..HIDDEN_SIZE_1 * HIDDEN_SIZE_2)
                .map(|_| rng.gen_range(-hidden1_range..=hidden1_range))
                .collect(),
            hidden1_bias: vec![0; HIDDEN_SIZE_2],
            
            // Terceira camada
            hidden2_weights: (0..HIDDEN_SIZE_2 * HIDDEN_SIZE_3)
                .map(|_| rng.gen_range(-hidden2_range..=hidden2_range))
                .collect(),
            hidden2_bias: vec![0; HIDDEN_SIZE_3],
            
            // Saída
            output_weights: (0..HIDDEN_SIZE_3)
                .map(|_| rng.gen_range(-output_range..=output_range))
                .collect(),
            output_bias: 0,
            
            // Escalas
            feature_scale: FEATURE_SCALE,
            hidden_scale: HIDDEN_SCALE,
            output_scale: OUTPUT_SCALE,
        }
    }
    
    /// Avaliação rápida usando acumuladores incrementais com arquitetura multicamada
    pub fn evaluate_incremental(&self, features: &NNUEAccumulator) -> i32 {
        // Primeira camada (já computada no accumulator)
        let mut hidden1_output = vec![0i32; HIDDEN_SIZE_2];
        
        // Segunda camada (hidden1 -> hidden2)
        for i in 0..HIDDEN_SIZE_2 {
            let mut sum = self.hidden1_bias[i] as i32;
            
            for j in 0..HIDDEN_SIZE_1 {
                let activated = features.accumulator[j].max(0) / self.feature_scale;
                sum += activated * self.hidden1_weights[j * HIDDEN_SIZE_2 + i] as i32;
            }
            
            hidden1_output[i] = sum.max(0); // ReLU
        }
        
        // Terceira camada (hidden2 -> hidden3)
        let mut hidden2_output = vec![0i32; HIDDEN_SIZE_3];
        
        for i in 0..HIDDEN_SIZE_3 {
            let mut sum = self.hidden2_bias[i] as i32;
            
            for j in 0..HIDDEN_SIZE_2 {
                let activated = hidden1_output[j] / self.hidden_scale;
                sum += activated * self.hidden2_weights[j * HIDDEN_SIZE_3 + i] as i32;
            }
            
            hidden2_output[i] = sum.max(0); // ReLU
        }
        
        // Camada de saída (hidden3 -> output)
        let mut final_sum = self.output_bias;
        
        for i in 0..HIDDEN_SIZE_3 {
            let activated = hidden2_output[i] / self.hidden_scale;
            final_sum += activated * self.output_weights[i] as i32;
        }
        
        // Normaliza para centipawns com escala melhorada
        (final_sum * 100) / self.output_scale
    }
    
    /// Salva rede em formato binário compacto
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::{BufWriter, Write};
        
        let mut writer = BufWriter::new(File::create(path)?);
        
        // Header melhorado
        writer.write_all(b"PLC2")?; // Pelanca NNUE v2 format
        writer.write_all(&(self.feature_scale as u32).to_le_bytes())?;
        writer.write_all(&(self.hidden_scale as u32).to_le_bytes())?;
        writer.write_all(&(self.output_scale as u32).to_le_bytes())?;
        
        // Feature layer
        for &weight in &self.feature_weights {
            writer.write_all(&weight.to_le_bytes())?;
        }
        for &bias in &self.feature_bias {
            writer.write_all(&bias.to_le_bytes())?;
        }
        
        // Hidden layer 1
        writer.write_all(&self.hidden1_weights.iter().map(|&x| x as u8).collect::<Vec<u8>>())?;
        for &bias in &self.hidden1_bias {
            writer.write_all(&bias.to_le_bytes())?;
        }
        
        // Hidden layer 2
        writer.write_all(&self.hidden2_weights.iter().map(|&x| x as u8).collect::<Vec<u8>>())?;
        for &bias in &self.hidden2_bias {
            writer.write_all(&bias.to_le_bytes())?;
        }
        
        // Output layer
        writer.write_all(&self.output_weights.iter().map(|&x| x as u8).collect::<Vec<u8>>())?;
        writer.write_all(&self.output_bias.to_le_bytes())?;
        
        Ok(())
    }
    
    /// Carrega rede de arquivo
    pub fn load(path: &str) -> std::io::Result<Self> {
        use std::fs::File;
        use std::io::{BufReader, Read};
        
        let mut reader = BufReader::new(File::open(path)?);
        
        // Verifica header
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        if &header != b"PLC2" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid Pelanca NNUE v2 file"
            ));
        }
        
        // Lê escalas
        let mut scale_bytes = [0u8; 4];
        reader.read_exact(&mut scale_bytes)?;
        let feature_scale = u32::from_le_bytes(scale_bytes) as i32;
        
        reader.read_exact(&mut scale_bytes)?;
        let hidden_scale = u32::from_le_bytes(scale_bytes) as i32;
        
        reader.read_exact(&mut scale_bytes)?;
        let output_scale = u32::from_le_bytes(scale_bytes) as i32;
        
        // Feature layer
        let mut feature_weights = vec![0i16; FEATURE_SIZE * HIDDEN_SIZE_1];
        for weight in &mut feature_weights {
            let mut bytes = [0u8; 2];
            reader.read_exact(&mut bytes)?;
            *weight = i16::from_le_bytes(bytes);
        }
        
        let mut feature_bias = vec![0i32; HIDDEN_SIZE_1];
        for bias in &mut feature_bias {
            let mut bytes = [0u8; 4];
            reader.read_exact(&mut bytes)?;
            *bias = i32::from_le_bytes(bytes);
        }
        
        // Hidden layer 1
        let mut hidden1_weights_u8 = vec![0u8; HIDDEN_SIZE_1 * HIDDEN_SIZE_2];
        reader.read_exact(&mut hidden1_weights_u8)?;
        let hidden1_weights: Vec<i8> = hidden1_weights_u8.iter().map(|&x| x as i8).collect();
        
        let mut hidden1_bias = vec![0i16; HIDDEN_SIZE_2];
        for bias in &mut hidden1_bias {
            let mut bytes = [0u8; 2];
            reader.read_exact(&mut bytes)?;
            *bias = i16::from_le_bytes(bytes);
        }
        
        // Hidden layer 2
        let mut hidden2_weights_u8 = vec![0u8; HIDDEN_SIZE_2 * HIDDEN_SIZE_3];
        reader.read_exact(&mut hidden2_weights_u8)?;
        let hidden2_weights: Vec<i8> = hidden2_weights_u8.iter().map(|&x| x as i8).collect();
        
        let mut hidden2_bias = vec![0i16; HIDDEN_SIZE_3];
        for bias in &mut hidden2_bias {
            let mut bytes = [0u8; 2];
            reader.read_exact(&mut bytes)?;
            *bias = i16::from_le_bytes(bytes);
        }
        
        // Output layer
        let mut output_weights_u8 = vec![0u8; HIDDEN_SIZE_3];
        reader.read_exact(&mut output_weights_u8)?;
        let output_weights: Vec<i8> = output_weights_u8.iter().map(|&x| x as i8).collect();
        
        let mut bias_bytes = [0u8; 4];
        reader.read_exact(&mut bias_bytes)?;
        let output_bias = i32::from_le_bytes(bias_bytes);
        
        Ok(Self {
            feature_weights,
            feature_bias,
            hidden1_weights,
            hidden1_bias,
            hidden2_weights,
            hidden2_bias,
            output_weights,
            output_bias,
            feature_scale,
            hidden_scale,
            output_scale,
        })
    }
}

impl Default for NNUE {
    fn default() -> Self {
        Self::new()
    }
}