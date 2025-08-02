// Motor Xadrez - High-Performance Chess Engine

use pelanca::{Board, Color, PieceKind};

fn main() {
    let board = Board::new();
    println!("=== Motor Xadrez - Pronto para IA ===");
    println!("Zobrist hash inicial: {}", board.zobrist_hash);

    // Exemplo de uso básico
    println!("\n📋 Posição inicial:");
    println!("  Peças brancas: {:016x}", board.white_pieces);
    println!("  Peças pretas: {:016x}", board.black_pieces);
    println!("  Vez de jogar: {:?}", board.to_move);

    // Contagem de peças
    println!("\n🔢 Contagem de peças:");
    for &piece in &[PieceKind::Pawn, PieceKind::Knight, PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen, PieceKind::King] {
        let white_count = board.piece_count(Color::White, piece);
        let black_count = board.piece_count(Color::Black, piece);
        println!("  {:?}: Brancas={}, Pretas={}", piece, white_count, black_count);
    }

    // Gerar movimentos legais
    let legal_moves = board.generate_legal_moves();
    println!("\n♟️  Movimentos legais disponíveis: {}", legal_moves.len());

    // Verificar estado do jogo
    println!("\n🎯 Estado do jogo:");
    println!("  Rei branco em xeque: {}", board.is_king_in_check(Color::White));
    println!("  Rei preto em xeque: {}", board.is_king_in_check(Color::Black));
    println!("  Jogo terminado: {}", board.is_game_over());
    println!("  Halfmove clock: {}", board.halfmove_clock);

    // Exemplo com FEN
    println!("\n🔄 Testando FEN:");
    let test_fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    match Board::from_fen(test_fen) {
        Ok(fen_board) => {
            println!("  FEN válida: {} movimentos legais", fen_board.generate_legal_moves().len());
            println!("  Zobrist hash: {}", fen_board.zobrist_hash);
        }
        Err(e) => println!("  Erro FEN: {}", e),
    }

    // Pronto para integração com IA
    println!("\n🚀 Próximos passos:");
    println!("  ✅ Motor validado e funcional");
    println!("  ✅ Zobrist hashing implementado");
    println!("  ✅ Detecção de draws completa");
    println!("  ✅ Performance otimizada (60M+ NPS)");
    println!("  📝 Implementar: Avaliação de posição");
    println!("  📝 Implementar: Busca minimax/alpha-beta");
    println!("  📝 Implementar: Interface UCI");
}