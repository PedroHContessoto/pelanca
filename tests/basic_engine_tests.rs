use pelanca::core::Board;
use pelanca::engine::perft::perft;

#[test]
fn initial_position_has_20_legal_moves() {
    let board = Board::new();
    let legal = board.generate_legal_moves();
    assert_eq!(legal.len(), 20, "Initial chess position must have exactly 20 legal moves");
}

#[test]
fn perft_depth_1_and_2_match_known_values() {
    let mut board = Board::new();
    assert_eq!(perft(&mut board, 1), 20);
    assert_eq!(perft(&mut board, 2), 400);
}

#[test]
fn initial_position_is_not_game_over() {
    let board = Board::new();
    assert!(!board.is_game_over(), "Initial position must not be terminal");
}
