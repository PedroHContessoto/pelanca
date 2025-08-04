// Avaliacao de posicao - Sistema de avaliacao completo para motor de xadrez
// Baseado em principios classicos com otimizacoes modernas

use crate::core::*;
use crate::utils::BitboardOps;

/// Valores das pecas (centipawns)
const PIECE_VALUES: [i16; 6] = [
    100,  // Pawn
    320,  // Knight  
    330,  // Bishop
    500,  // Rook
    900,  // Queen
    20000 // King (valor alto para evitar trocas)
];

/// Bonus por par de bispos
const BISHOP_PAIR_BONUS: i16 = 30;

/// Penalidade por peoes dobrados
const DOUBLED_PAWN_PENALTY: i16 = -20;

/// Bonus por peoes passados (por rank)
const PASSED_PAWN_BONUS: [i16; 8] = [0, 10, 20, 35, 55, 80, 120, 0];

/// Tabelas de posicao (Piece-Square Tables) para fase de abertura
const PAWN_PST_OPENING: [i16; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
     50,  50,  50,  50,  50,  50,  50,  50,
     10,  10,  20,  30,  30,  20,  10,  10,
      5,   5,  10,  25,  25,  10,   5,   5,
      0,   0,   0,  20,  20,   0,   0,   0,
      5,  -5, -10,   0,   0, -10,  -5,   5,
      5,  10,  10, -20, -20,  10,  10,   5,
      0,   0,   0,   0,   0,   0,   0,   0,
];

const KNIGHT_PST_OPENING: [i16; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,   0,   0,   0,   0, -20, -40,
    -30,   0,  10,  15,  15,  10,   0, -30,
    -30,   5,  15,  20,  20,  15,   5, -30,
    -30,   0,  15,  20,  20,  15,   0, -30,
    -30,   5,  10,  15,  15,  10,   5, -30,
    -40, -20,   0,   5,   5,   0, -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50,
];

const BISHOP_PST_OPENING: [i16; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -10,   0,   5,  10,  10,   5,   0, -10,
    -10,   5,   5,  10,  10,   5,   5, -10,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,  10,  10,  10,  10,  10,  10, -10,
    -10,   5,   0,   0,   0,   0,   5, -10,
    -20, -10, -10, -10, -10, -10, -10, -20,
];

const ROOK_PST_OPENING: [i16; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
      5,  10,  10,  10,  10,  10,  10,   5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
      0,   0,   0,   5,   5,   0,   0,   0,
];

const QUEEN_PST_OPENING: [i16; 64] = [
    -20, -10, -10,  -5,  -5, -10, -10, -20,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -10,   0,   5,   5,   5,   5,   0, -10,
     -5,   0,   5,   5,   5,   5,   0,  -5,
      0,   0,   5,   5,   5,   5,   0,  -5,
    -10,   5,   5,   5,   5,   5,   0, -10,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -20, -10, -10,  -5,  -5, -10, -10, -20,
];

const KING_PST_OPENING: [i16; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -10, -20, -20, -20, -20, -20, -20, -10,
     20,  20,   0,   0,   0,   0,  20,  20,
     20,  30,  10,   0,   0,  10,  30,  20,
];

/// Tabelas de posicao para final de jogo
const KING_PST_ENDGAME: [i16; 64] = [
    -50, -40, -30, -20, -20, -30, -40, -50,
    -30, -20, -10,   0,   0, -10, -20, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -30,   0,   0,   0,   0, -30, -30,
    -50, -30, -30, -30, -30, -30, -30, -50,
];

/// Estrutura para avaliacao progressiva
pub struct Evaluator;

impl Evaluator {
    /// Avaliacao principal da posicao
    pub fn evaluate(board: &Board) -> i16 {
        let mut score = 0;
        
        // Material base
        score += Self::evaluate_material(board);
        
        // Avaliacao posicional
        score += Self::evaluate_piece_square_tables(board);
        
        // Estrutura de peoes
        score += Self::evaluate_pawn_structure(board);
        
        // Seguranca do rei
        score += Self::evaluate_king_safety(board);
        
        // Mobilidade das pecas
        score += Self::evaluate_mobility(board);
        
        // Bonus especiais
        score += Self::evaluate_bishops_pair(board);
        score += Self::evaluate_rooks_on_open_files(board);
        
        // Retorna score do ponto de vista do jogador atual
        if board.to_move == Color::White { score } else { -score }
    }

    /// Avaliacao de material
    fn evaluate_material(board: &Board) -> i16 {
        let mut score = 0;
        
        for piece_kind in [PieceKind::Pawn, PieceKind::Knight, PieceKind::Bishop, 
                          PieceKind::Rook, PieceKind::Queen] {
            let white_count = board.piece_count(Color::White, piece_kind) as i16;
            let black_count = board.piece_count(Color::Black, piece_kind) as i16;
            
            let piece_value = PIECE_VALUES[Self::piece_kind_to_index(piece_kind)];
            score += (white_count - black_count) * piece_value;
        }
        
        score
    }

    /// Avaliacao usando tabelas de posicao
    fn evaluate_piece_square_tables(board: &Board) -> i16 {
        let mut score = 0;
        let is_endgame = Self::is_endgame(board);
        
        // Avalia cada tipo de peca
        score += Self::evaluate_piece_pst(board, PieceKind::Pawn, &PAWN_PST_OPENING, &PAWN_PST_OPENING);
        score += Self::evaluate_piece_pst(board, PieceKind::Knight, &KNIGHT_PST_OPENING, &KNIGHT_PST_OPENING);
        score += Self::evaluate_piece_pst(board, PieceKind::Bishop, &BISHOP_PST_OPENING, &BISHOP_PST_OPENING);
        score += Self::evaluate_piece_pst(board, PieceKind::Rook, &ROOK_PST_OPENING, &ROOK_PST_OPENING);
        score += Self::evaluate_piece_pst(board, PieceKind::Queen, &QUEEN_PST_OPENING, &QUEEN_PST_OPENING);
        
        // Rei usa tabelas diferentes para abertura/final
        let king_opening = &KING_PST_OPENING;
        let king_endgame = &KING_PST_ENDGAME;
        score += Self::evaluate_piece_pst(board, PieceKind::King, king_opening, king_endgame);
        
        score
    }

    /// Avalia uma peca especifica usando PST
    fn evaluate_piece_pst(board: &Board, piece_kind: PieceKind, opening_pst: &[i16; 64], endgame_pst: &[i16; 64]) -> i16 {
        let mut score = 0;
        let is_endgame = Self::is_endgame(board);
        
        let piece_bb = Self::get_piece_bitboard(board, piece_kind);
        
        // Pecas brancas
        let white_pieces = piece_bb & board.white_pieces;
        for square in white_pieces.iter_squares() {
            let pst_score = if is_endgame { endgame_pst[square as usize] } else { opening_pst[square as usize] };
            score += pst_score;
        }
        
        // Pecas pretas (flip vertical)
        let black_pieces = piece_bb & board.black_pieces;
        for square in black_pieces.iter_squares() {
            let flipped_square = square ^ 56; // Flip vertical
            let pst_score = if is_endgame { endgame_pst[flipped_square as usize] } else { opening_pst[flipped_square as usize] };
            score -= pst_score;
        }
        
        score
    }

    /// Avaliacao da estrutura de peoes
    fn evaluate_pawn_structure(board: &Board) -> i16 {
        let mut score = 0;
        
        // Peoes dobrados
        score += Self::evaluate_doubled_pawns(board);
        
        // Peoes passados
        score += Self::evaluate_passed_pawns(board);
        
        // Peoes isolados
        score += Self::evaluate_isolated_pawns(board);
        
        score
    }

    /// Avalia peoes dobrados
    fn evaluate_doubled_pawns(board: &Board) -> i16 {
        let mut score = 0;
        
        for file in 0..8 {
            let file_mask = 0x0101010101010101u64 << file;
            
            let white_pawns_on_file = (board.pawns & board.white_pieces & file_mask).popcount_fast();
            let black_pawns_on_file = (board.pawns & board.black_pieces & file_mask).popcount_fast();
            
            if white_pawns_on_file > 1 {
                score += DOUBLED_PAWN_PENALTY * (white_pawns_on_file as i16 - 1);
            }
            if black_pawns_on_file > 1 {
                score -= DOUBLED_PAWN_PENALTY * (black_pawns_on_file as i16 - 1);
            }
        }
        
        score
    }

    /// Avalia peoes passados
    fn evaluate_passed_pawns(board: &Board) -> i16 {
        let mut score = 0;
        
        // Implementacao simplificada - versao completa seria mais complexa
        let white_pawns = board.pawns & board.white_pieces;
        let black_pawns = board.pawns & board.black_pieces;
        
        for square in white_pawns.iter_squares() {
            let rank = square / 8;
            let file = square % 8;
            
            // Verifica se e passado (implementacao basica)
            let front_mask = Self::get_front_span_mask(square, Color::White);
            let file_masks = Self::get_adjacent_files_mask(file);
            
            if (black_pawns & front_mask & file_masks) == 0 {
                score += PASSED_PAWN_BONUS[rank as usize];
            }
        }
        
        for square in black_pawns.iter_squares() {
            let rank = square / 8;
            let file = square % 8;
            
            let front_mask = Self::get_front_span_mask(square, Color::Black);
            let file_masks = Self::get_adjacent_files_mask(file);
            
            if (white_pawns & front_mask & file_masks) == 0 {
                score -= PASSED_PAWN_BONUS[7 - rank as usize];
            }
        }
        
        score
    }

    /// Avalia peoes isolados
    fn evaluate_isolated_pawns(board: &Board) -> i16 {
        let mut score = 0;
        const ISOLATED_PAWN_PENALTY: i16 = -15;
        
        for file in 0..8 {
            let file_mask = 0x0101010101010101u64 << file;
            let adjacent_files = Self::get_adjacent_files_mask(file);
            
            // Brancas
            if (board.pawns & board.white_pieces & file_mask) != 0 {
                if (board.pawns & board.white_pieces & adjacent_files) == 0 {
                    score += ISOLATED_PAWN_PENALTY;
                }
            }
            
            // Pretas
            if (board.pawns & board.black_pieces & file_mask) != 0 {
                if (board.pawns & board.black_pieces & adjacent_files) == 0 {
                    score -= ISOLATED_PAWN_PENALTY;
                }
            }
        }
        
        score
    }

    /// Avaliacao da seguranca do rei
    fn evaluate_king_safety(board: &Board) -> i16 {
        let mut score = 0;
        const KING_SAFETY_BONUS: i16 = 20;
        
        // Implementacao basica - verifica se rei esta seguro atras de peoes
        let white_king_square = (board.kings & board.white_pieces).lsb_fast() as u8;
        let black_king_square = (board.kings & board.black_pieces).lsb_fast() as u8;
        
        // Peoes na frente do rei branco
        if white_king_square < 56 {
            let front_squares = Self::get_king_shield_mask(white_king_square);
            let shield_pawns = (front_squares & board.pawns & board.white_pieces).popcount_fast();
            score += shield_pawns as i16 * KING_SAFETY_BONUS;
        }
        
        // Peoes na frente do rei preto
        if black_king_square > 7 {
            let front_squares = Self::get_king_shield_mask(black_king_square);
            let shield_pawns = (front_squares & board.pawns & board.black_pieces).popcount_fast();
            score -= shield_pawns as i16 * KING_SAFETY_BONUS;
        }
        
        score
    }

    /// Avaliacao da mobilidade das pecas
    fn evaluate_mobility(board: &Board) -> i16 {
        let mut score = 0;
        const MOBILITY_BONUS: i16 = 2;
        
        // Mobilidade dos cavalos
        let white_knights = board.knights & board.white_pieces;
        let black_knights = board.knights & board.black_pieces;
        
        for square in white_knights.iter_squares() {
            let attacks = crate::moves::knight::get_knight_attacks(square);
            let mobility = (attacks & !board.white_pieces).popcount_fast();
            score += mobility as i16 * MOBILITY_BONUS;
        }
        
        for square in black_knights.iter_squares() {
            let attacks = crate::moves::knight::get_knight_attacks(square);
            let mobility = (attacks & !board.black_pieces).popcount_fast();
            score -= mobility as i16 * MOBILITY_BONUS;
        }
        
        score
    }

    /// Bonus por par de bispos
    fn evaluate_bishops_pair(board: &Board) -> i16 {
        let mut score = 0;
        
        let white_bishops = (board.bishops & board.white_pieces).popcount_fast();
        let black_bishops = (board.bishops & board.black_pieces).popcount_fast();
        
        if white_bishops >= 2 {
            score += BISHOP_PAIR_BONUS;
        }
        if black_bishops >= 2 {
            score -= BISHOP_PAIR_BONUS;
        }
        
        score
    }

    /// Bonus por torres em colunas abertas
    fn evaluate_rooks_on_open_files(board: &Board) -> i16 {
        let mut score = 0;
        const OPEN_FILE_BONUS: i16 = 25;
        const SEMI_OPEN_FILE_BONUS: i16 = 15;
        
        let white_rooks = board.rooks & board.white_pieces;
        let black_rooks = board.rooks & board.black_pieces;
        
        for square in white_rooks.iter_squares() {
            let file = square % 8;
            let file_mask = 0x0101010101010101u64 << file;
            
            let white_pawns_on_file = (board.pawns & board.white_pieces & file_mask).popcount_fast();
            let black_pawns_on_file = (board.pawns & board.black_pieces & file_mask).popcount_fast();
            
            if white_pawns_on_file == 0 && black_pawns_on_file == 0 {
                score += OPEN_FILE_BONUS; // Coluna aberta
            } else if white_pawns_on_file == 0 {
                score += SEMI_OPEN_FILE_BONUS; // Semi-aberta
            }
        }
        
        for square in black_rooks.iter_squares() {
            let file = square % 8;
            let file_mask = 0x0101010101010101u64 << file;
            
            let white_pawns_on_file = (board.pawns & board.white_pieces & file_mask).popcount_fast();
            let black_pawns_on_file = (board.pawns & board.black_pieces & file_mask).popcount_fast();
            
            if white_pawns_on_file == 0 && black_pawns_on_file == 0 {
                score -= OPEN_FILE_BONUS;
            } else if black_pawns_on_file == 0 {
                score -= SEMI_OPEN_FILE_BONUS;
            }
        }
        
        score
    }

    // Funcoes auxiliares

    fn is_endgame(board: &Board) -> bool {
        let total_material = 
            board.piece_count(Color::White, PieceKind::Queen) +
            board.piece_count(Color::Black, PieceKind::Queen) +
            board.piece_count(Color::White, PieceKind::Rook) +
            board.piece_count(Color::Black, PieceKind::Rook) +
            board.piece_count(Color::White, PieceKind::Bishop) +
            board.piece_count(Color::Black, PieceKind::Bishop) +
            board.piece_count(Color::White, PieceKind::Knight) +
            board.piece_count(Color::Black, PieceKind::Knight);
        
        total_material <= 10 // Heuristica simples para endgame
    }

    fn get_piece_bitboard(board: &Board, piece_kind: PieceKind) -> Bitboard {
        match piece_kind {
            PieceKind::Pawn => board.pawns,
            PieceKind::Knight => board.knights,
            PieceKind::Bishop => board.bishops,
            PieceKind::Rook => board.rooks,
            PieceKind::Queen => board.queens,
            PieceKind::King => board.kings,
        }
    }

    fn piece_kind_to_index(piece_kind: PieceKind) -> usize {
        match piece_kind {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5,
        }
    }

    fn get_front_span_mask(square: u8, color: Color) -> Bitboard {
        let file = square % 8;
        let rank = square / 8;
        let file_mask = 0x0101010101010101u64 << file;
        
        match color {
            Color::White => {
                let front_ranks = 0xFFFFFFFFFFFFFFFFu64 << ((rank + 1) * 8);
                file_mask & front_ranks
            }
            Color::Black => {
                let front_ranks = (1u64 << (rank * 8)) - 1;
                file_mask & front_ranks
            }
        }
    }

    fn get_adjacent_files_mask(file: u8) -> Bitboard {
        let mut mask = 0u64;
        if file > 0 {
            mask |= 0x0101010101010101u64 << (file - 1);
        }
        if file < 7 {
            mask |= 0x0101010101010101u64 << (file + 1);
        }
        mask
    }

    fn get_king_shield_mask(king_square: u8) -> Bitboard {
        let file = king_square % 8;
        let rank = king_square / 8;
        let mut mask = 0u64;
        
        // Casas na frente do rei (simplificado)
        for f in file.saturating_sub(1)..=(file + 1).min(7) {
            if rank < 7 {
                mask |= 1u64 << ((rank + 1) * 8 + f);
            }
            if rank < 6 {
                mask |= 1u64 << ((rank + 2) * 8 + f);
            }
        }
        
        mask
    }
}