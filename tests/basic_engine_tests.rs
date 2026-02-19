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

#[test]
fn from_fen_parses_start_position_without_panicking() {
    let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
        .expect("valid start position FEN should parse");

    assert_eq!(board.castling_rights, 0b1111);
    assert_eq!(board.generate_legal_moves().len(), 20);
}

#[test]
fn from_fen_rejects_rows_with_invalid_square_count() {
    let invalid = Board::from_fen("8/8/8/8/8/8/8/9 w - - 0 1");
    assert!(invalid.is_err(), "FEN rows with more than 8 squares must be rejected");
}


#[test]
fn search_respects_nodes_limit() {
    use pelanca::engine::eval::PstEvaluator;
    use pelanca::engine::search::NegamaxSearcher;
    use pelanca::engine::{Searcher, SearchConfig};

    let board = Board::new();
    let mut searcher = NegamaxSearcher::new(PstEvaluator);
    let config = SearchConfig {
        max_depth: 8,
        nodes_limit: Some(1_000),
        ..Default::default()
    };

    let result = searcher.search(&board, &config);
    assert!(result.nodes_searched <= 3_000, "search should stop around nodes limit");
}
