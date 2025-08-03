// Sistema de avaliação de posições com tabelas PST (Piece-Square Tables)

use crate::core::*;

/// Avaliação principal da posição
pub fn evaluate(board: &Board) -> i32 {
    let mut score = 0;

    // Material
    score += evaluate_material(board);

    // Posição das peças (PST)
    score += evaluate_piece_positions(board);

    // Estrutura de peões
    score += evaluate_pawn_structure(board);

    // Mobilidade
    score += evaluate_mobility(board);

    // Segurança do rei
    score += evaluate_king_safety(board);

    // Retorna do ponto de vista do jogador atual
    if board.to_move == Color::White {
        score
    } else {
        -score
    }
}

/// Avalia material básico
fn evaluate_material(board: &Board) -> i32 {
    let mut score = 0;

    // Valores das peças
    score += (board.piece_count(Color::White, PieceKind::Pawn) as i32 -
        board.piece_count(Color::Black, PieceKind::Pawn) as i32) * 100;
    score += (board.piece_count(Color::White, PieceKind::Knight) as i32 -
        board.piece_count(Color::Black, PieceKind::Knight) as i32) * 320;
    score += (board.piece_count(Color::White, PieceKind::Bishop) as i32 -
        board.piece_count(Color::Black, PieceKind::Bishop) as i32) * 330;
    score += (board.piece_count(Color::White, PieceKind::Rook) as i32 -
        board.piece_count(Color::Black, PieceKind::Rook) as i32) * 500;
    score += (board.piece_count(Color::White, PieceKind::Queen) as i32 -
        board.piece_count(Color::Black, PieceKind::Queen) as i32) * 900;

    // Bônus do par de bispos
    if board.piece_count(Color::White, PieceKind::Bishop) >= 2 {
        score += 30;
    }
    if board.piece_count(Color::Black, PieceKind::Bishop) >= 2 {
        score -= 30;
    }

    score
}

/// Piece-Square Tables para avaliação posicional
const PAWN_PST: [i32; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
    5,  5, 10, 25, 25, 10,  5,  5,
    0,  0,  0, 20, 20,  0,  0,  0,
    5, -5,-10,  0,  0,-10, -5,  5,
    5, 10, 10,-20,-20, 10, 10,  5,
    0,  0,  0,  0,  0,  0,  0,  0
];

const KNIGHT_PST: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50,
];

const BISHOP_PST: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20,
];

const ROOK_PST: [i32; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,
    5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    0,  0,  0,  5,  5,  0,  0,  0
];

const QUEEN_PST: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
    -5,  0,  5,  5,  5,  5,  0, -5,
    0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

const KING_PST_MIDDLE: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
    20, 20,  0,  0,  0,  0, 20, 20,
    20, 30, 10,  0,  0, 10, 30, 20
];

const KING_PST_END: [i32; 64] = [
    -50,-40,-30,-20,-20,-30,-40,-50,
    -30,-20,-10,  0,  0,-10,-20,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-30,  0,  0,  0,  0,-30,-30,
    -50,-30,-30,-30,-30,-30,-30,-50
];

/// Avalia posições das peças usando PST
fn evaluate_piece_positions(board: &Board) -> i32 {
    let mut score = 0;
    let is_endgame = is_endgame_phase(board);

    // Avalia cada tipo de peça
    score += evaluate_piece_type_positions(board, PieceKind::Pawn, &PAWN_PST);
    score += evaluate_piece_type_positions(board, PieceKind::Knight, &KNIGHT_PST);
    score += evaluate_piece_type_positions(board, PieceKind::Bishop, &BISHOP_PST);
    score += evaluate_piece_type_positions(board, PieceKind::Rook, &ROOK_PST);
    score += evaluate_piece_type_positions(board, PieceKind::Queen, &QUEEN_PST);

    // Rei usa tabela diferente para final
    let king_pst = if is_endgame { &KING_PST_END } else { &KING_PST_MIDDLE };
    score += evaluate_piece_type_positions(board, PieceKind::King, king_pst);

    score
}

/// Avalia posições de um tipo específico de peça
fn evaluate_piece_type_positions(board: &Board, piece_type: PieceKind, pst: &[i32; 64]) -> i32 {
    let mut score = 0;

    let piece_bb = match piece_type {
        PieceKind::Pawn => board.pawns,
        PieceKind::Knight => board.knights,
        PieceKind::Bishop => board.bishops,
        PieceKind::Rook => board.rooks,
        PieceKind::Queen => board.queens,
        PieceKind::King => board.kings,
    };

    // Peças brancas
    let mut white_pieces = piece_bb & board.white_pieces;
    while white_pieces != 0 {
        let sq = white_pieces.trailing_zeros() as usize;
        score += pst[sq];
        white_pieces &= white_pieces - 1;
    }

    // Peças pretas (espelhadas)
    let mut black_pieces = piece_bb & board.black_pieces;
    while black_pieces != 0 {
        let sq = black_pieces.trailing_zeros() as usize;
        let mirrored_sq = sq ^ 56; // Espelha verticalmente
        score -= pst[mirrored_sq];
        black_pieces &= black_pieces - 1;
    }

    score
}

/// Avalia estrutura de peões
fn evaluate_pawn_structure(board: &Board) -> i32 {
    let mut score = 0;

    let white_pawns = board.pawns & board.white_pieces;
    let black_pawns = board.pawns & board.black_pieces;

    // Peões passados
    if board.has_passed_pawn(Color::White) {
        score += 50;
    }
    if board.has_passed_pawn(Color::Black) {
        score -= 50;
    }

    // Peões isolados
    score -= count_isolated_pawns(white_pawns) * 10;
    score += count_isolated_pawns(black_pawns) * 10;

    // Peões dobrados
    score -= count_doubled_pawns(white_pawns) * 15;
    score += count_doubled_pawns(black_pawns) * 15;

    score
}

/// Conta peões isolados
fn count_isolated_pawns(pawns: Bitboard) -> i32 {
    let mut count = 0;
    let file_masks = [
        0x0101010101010101u64,
        0x0202020202020202u64,
        0x0404040404040404u64,
        0x0808080808080808u64,
        0x1010101010101010u64,
        0x2020202020202020u64,
        0x4040404040404040u64,
        0x8080808080808080u64,
    ];

    for i in 0..8 {
        if (pawns & file_masks[i]) != 0 {
            let adjacent_files = if i > 0 { file_masks[i-1] } else { 0 } |
                if i < 7 { file_masks[i+1] } else { 0 };

            if (pawns & adjacent_files) == 0 {
                count += (pawns & file_masks[i]).count_ones() as i32;
            }
        }
    }

    count
}

/// Conta peões dobrados
fn count_doubled_pawns(pawns: Bitboard) -> i32 {
    let mut count = 0;
    let file_masks = [
        0x0101010101010101u64,
        0x0202020202020202u64,
        0x0404040404040404u64,
        0x0808080808080808u64,
        0x1010101010101010u64,
        0x2020202020202020u64,
        0x4040404040404040u64,
        0x8080808080808080u64,
    ];

    for &mask in &file_masks {
        let pawns_on_file = (pawns & mask).count_ones();
        if pawns_on_file > 1 {
            count += (pawns_on_file - 1) as i32;
        }
    }

    count
}

/// Avalia mobilidade básica
fn evaluate_mobility(board: &Board) -> i32 {
    let mut score = 0;
    let all_pieces = board.white_pieces | board.black_pieces;

    // Mobilidade de cavalos
    let mut white_knights = board.knights & board.white_pieces;
    while white_knights != 0 {
        let sq = white_knights.trailing_zeros() as u8;
        let moves = crate::moves::knight::get_knight_attacks(sq) & !board.white_pieces;
        score += moves.count_ones() as i32 * 4;
        white_knights &= white_knights - 1;
    }

    let mut black_knights = board.knights & board.black_pieces;
    while black_knights != 0 {
        let sq = black_knights.trailing_zeros() as u8;
        let moves = crate::moves::knight::get_knight_attacks(sq) & !board.black_pieces;
        score -= moves.count_ones() as i32 * 4;
        black_knights &= black_knights - 1;
    }

    // Mobilidade de bispos
    let mut white_bishops = board.bishops & board.white_pieces;
    while white_bishops != 0 {
        let sq = white_bishops.trailing_zeros() as u8;
        let moves = crate::moves::sliding::get_bishop_attacks(sq, all_pieces) & !board.white_pieces;
        score += moves.count_ones() as i32 * 3;
        white_bishops &= white_bishops - 1;
    }

    let mut black_bishops = board.bishops & board.black_pieces;
    while black_bishops != 0 {
        let sq = black_bishops.trailing_zeros() as u8;
        let moves = crate::moves::sliding::get_bishop_attacks(sq, all_pieces) & !board.black_pieces;
        score -= moves.count_ones() as i32 * 3;
        black_bishops &= black_bishops - 1;
    }

    score
}

/// Avalia segurança do rei
fn evaluate_king_safety(board: &Board) -> i32 {
    let mut score = 0;

    // Encontra posições dos reis
    let white_king_sq = (board.kings & board.white_pieces).trailing_zeros() as u8;
    let black_king_sq = (board.kings & board.black_pieces).trailing_zeros() as u8;

    // Penaliza rei no centro durante abertura/meio-jogo
    if !is_endgame_phase(board) {
        // Rei branco
        let white_king_file = white_king_sq % 8;
        let white_king_rank = white_king_sq / 8;

        if white_king_file >= 2 && white_king_file <= 5 && white_king_rank >= 2 {
            score -= 30;
        }

        // Rei preto
        let black_king_file = black_king_sq % 8;
        let black_king_rank = black_king_sq / 8;

        if black_king_file >= 2 && black_king_file <= 5 && black_king_rank <= 5 {
            score += 30;
        }

        // Bônus por roque
        if board.castling_rights == 0 {
            // Já rocou ou perdeu direitos
            if white_king_file <= 2 || white_king_file >= 6 {
                score += 20; // Provavelmente rocou
            }
            if black_king_file <= 2 || black_king_file >= 6 {
                score -= 20; // Provavelmente rocou
            }
        }
    }

    // Escudo de peões
    score += evaluate_pawn_shield(board, white_king_sq, Color::White);
    score -= evaluate_pawn_shield(board, black_king_sq, Color::Black);

    score
}

/// Avalia escudo de peões ao redor do rei
fn evaluate_pawn_shield(board: &Board, king_sq: u8, color: Color) -> i32 {
    let mut score = 0;
    let pawns = board.pawns & if color == Color::White { board.white_pieces } else { board.black_pieces };

    let king_file = king_sq % 8;
    let king_rank = king_sq / 8;

    // Define área ao redor do rei
    let shield_ranks = if color == Color::White {
        [king_rank + 1, king_rank + 2]
    } else {
        [king_rank.saturating_sub(1), king_rank.saturating_sub(2)]
    };

    for &rank in &shield_ranks {
        if rank < 8 {
            for file in king_file.saturating_sub(1)..=(king_file + 1).min(7) {
                let sq = rank * 8 + file;
                if (pawns & (1u64 << sq)) != 0 {
                    score += 10;
                }
            }
        }
    }

    score
}

/// Determina se estamos na fase final do jogo
fn is_endgame_phase(board: &Board) -> bool {
    let queens = (board.queens & (board.white_pieces | board.black_pieces)).count_ones();
    let rooks = (board.rooks & (board.white_pieces | board.black_pieces)).count_ones();
    let minors = ((board.bishops | board.knights) & (board.white_pieces | board.black_pieces)).count_ones();

    queens == 0 || (queens == 2 && rooks <= 2 && minors <= 4)
}