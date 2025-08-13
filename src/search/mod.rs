// Search Module - High-Performance Chess Search Algorithms
// Autor: Pedro Contessoto

pub mod evaluation;
pub mod transposition;
pub mod alphabeta;
pub mod ordering;

pub use evaluation::*;
pub use transposition::*;
pub use alphabeta::*;
pub use ordering::*;

// Constantes do search
pub const MAX_PLY: usize = 64;
pub const MATE_SCORE: i32 = 100000; // Aumentado para garantir que mate seja detectado
pub const MATE_IN_MAX: i32 = MATE_SCORE - MAX_PLY as i32;

// Tipos auxiliares
pub type Score = i32;
pub type Depth = u8;
pub type Ply = u8;