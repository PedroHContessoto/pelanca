// Estruturas auxiliares da rede NNUE - focado em compatibilidade com o sistema existente

use crate::core::Color;

/// Implementações adicionais para integração
impl From<Color> for u8 {
    fn from(color: Color) -> u8 {
        match color {
            Color::White => 0,
            Color::Black => 1,
        }
    }
}

impl Color {
    pub fn to_le_bytes(self) -> [u8; 1] {
        [self as u8]
    }
}