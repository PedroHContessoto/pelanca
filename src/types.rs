// Ficheiro: src/types.rs
// Descrição: Módulo para as definições de tipos de dados fundamentais do jogo.

// Um Bitboard é um inteiro de 64 bits sem sinal. Cada bit representa uma casa.
// Bit 0 = a1, Bit 1 = b1, ..., Bit 63 = h8.
pub type Bitboard = u64;

// Enum para representar a cor de uma peça ou de um jogador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Black,
}

impl std::ops::Not for Color {
    type Output = Color;

    fn not(self) -> Self::Output {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

// Enum para representar o tipo de uma peça de xadrez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceKind {
    pub fn value(&self) -> i32 {
        match self {
            PieceKind::Pawn   => 100,
            PieceKind::Knight => 320,
            PieceKind::Bishop => 330,
            PieceKind::Rook   => 500,
            PieceKind::Queen  => 900,
            PieceKind::King   => 20000, // Valor alto para evitar trocas
        }
    }
}

// ============================================================================
// COPY-MAKE OPTIMIZATION STRUCTURES
// ============================================================================

/// Estrutura para armazenar o estado do tabuleiro antes de um movimento
/// Usado para copy-make optimization (make/unmake rápido)
#[derive(Debug, Clone, Copy)]
pub struct UndoInfo {
    pub captured_piece: Option<PieceKind>,
    pub captured_square: u8,
    pub old_castling_rights: u8,
    pub old_en_passant_target: Option<u8>,
    pub old_halfmove_clock: u16,
    pub old_zobrist_hash: u64,
    pub old_white_king_in_check: bool,
    pub old_black_king_in_check: bool,
}


// Struct para representar uma peça no tabuleiro, combinando o tipo e a cor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    pub color: Color,
}

impl Piece {
    pub fn new(kind: PieceKind, color: Color) -> Self {
        Piece { kind, color }
    }

    pub fn piece_type(&self) -> PieceKind {
        self.kind
    }
}

// Struct para representar um lance no jogo.
// Guarda a casa de origem e a de destino.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub promotion: Option<PieceKind>,
    pub is_castling: bool,
    pub is_en_passant: bool,
}

// Adicione esta implementação para a struct Move
impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let from = to_algebraic(self.from);
        let to = to_algebraic(self.to);
        if let Some(p) = self.promotion {
            write!(f, "{}{}{}", from, to, piece_to_char(p))
        } else {
            write!(f, "{}{}", from, to)
        }
    }
}

// Adicione estas duas funções auxiliares no mesmo ficheiro
fn to_algebraic(sq: u8) -> String {
    let file = (sq % 8) as u8 + b'a';
    let rank = (sq / 8) as u8 + b'1';
    format!("{}{}", file as char, rank as char)
}

fn piece_to_char(p: PieceKind) -> char {
    match p {
        PieceKind::Queen => 'q',
        PieceKind::Rook => 'r',
        PieceKind::Bishop => 'b',
        PieceKind::Knight => 'n',
        _ => ' ',
    }
}