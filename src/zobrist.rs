// Zobrist hashing para detecção de repetições
use crate::types::*;

pub struct ZobristKeys {
    pub pieces: [[[u64; 64]; 6]; 2],  // [color][piece_type][square]
    pub castling: [u64; 16],          // Para direitos de roque
    pub en_passant: [u64; 8],         // Para en passant por coluna
    pub side_to_move: u64,            // Para quem joga
}

impl ZobristKeys {
    pub fn new() -> Self {
        let mut keys = ZobristKeys {
            pieces: [[[0; 64]; 6]; 2],
            castling: [0; 16],
            en_passant: [0; 8],
            side_to_move: 0,
        };

        // Gera chaves pseudo-aleatórias determinísticas
        let mut counter = 0u64;

        for color in 0..2 {
            for piece in 0..6 {
                for square in 0..64 {
                    keys.pieces[color][piece][square] = Self::hash_value(counter);
                    counter += 1;
                }
            }
        }

        for i in 0..16 {
            keys.castling[i] = Self::hash_value(counter);
            counter += 1;
        }

        for i in 0..8 {
            keys.en_passant[i] = Self::hash_value(counter);
            counter += 1;
        }

        keys.side_to_move = Self::hash_value(counter);

        keys
    }

    fn hash_value(seed: u64) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        hasher.finish()
    }
}

// Instância global das chaves Zobrist
lazy_static::lazy_static! {
    pub static ref ZOBRIST_KEYS: ZobristKeys = ZobristKeys::new();
}

pub fn piece_to_index(piece_kind: PieceKind) -> usize {
    match piece_kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    }
}

pub fn color_to_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}