#!/usr/bin/env python3
"""
Pelanca NNUE - Data Generation
Generates training positions using python-chess + Stockfish.
Outputs compact binary format for fast training.

Usage (Kaggle):
    !apt-get install -y stockfish
    !python generate_data.py --stockfish /usr/games/stockfish --num-positions 20000000 --output data.bin

Usage (local):
    python generate_data.py --stockfish ./stockfish --num-positions 1000000 --output data.bin
"""

import argparse
import chess
import chess.engine
import chess.pgn
import io
import os
import random
import struct
import time
from pathlib import Path

# Feature encoding: must match Rust exactly
# feature_index = color * 384 + piece_type * 64 + square
# color: White=0, Black=1
# piece_type: Pawn=0, Knight=1, Bishop=2, Rook=3, Queen=4, King=5
# square: a1=0, b1=1, ..., h8=63

PIECE_TYPE_MAP = {
    chess.PAWN: 0,
    chess.KNIGHT: 1,
    chess.BISHOP: 2,
    chess.ROOK: 3,
    chess.QUEEN: 4,
    chess.KING: 5,
}


def board_to_features(board: chess.Board) -> list[int]:
    """Extract active feature indices from a python-chess Board."""
    features = []
    for sq in chess.SQUARES:
        piece = board.piece_at(sq)
        if piece is not None:
            color = 0 if piece.color == chess.WHITE else 1
            piece_type = PIECE_TYPE_MAP[piece.piece_type]
            idx = color * 384 + piece_type * 64 + sq
            features.append(idx)
    return features


def write_position(f, board: chess.Board, score_cp: int):
    """Write a single position in binary format.
    Format:
        u8  side_to_move (0=White, 1=Black)
        u8  num_features
        u16 feature_indices[num_features]
        i16 score (centipawns, from White's perspective)
    """
    stm = 0 if board.turn == chess.WHITE else 1
    features = board_to_features(board)
    n = len(features)
    score_clamped = max(-4000, min(4000, score_cp))

    f.write(struct.pack('<B', stm))
    f.write(struct.pack('<B', n))
    for idx in features:
        f.write(struct.pack('<H', idx))
    f.write(struct.pack('<h', score_clamped))


def generate_random_positions(engine, num_positions: int, output_path: str,
                              depth: int = 10, batch_log: int = 10000):
    """Generate positions by playing random games and evaluating with Stockfish."""
    written = 0
    games_played = 0
    start_time = time.time()

    with open(output_path, 'wb') as f:
        while written < num_positions:
            board = chess.Board()
            # Play a random game
            move_count = 0
            positions_this_game = []

            while not board.is_game_over() and move_count < 300:
                legal_moves = list(board.legal_moves)
                if not legal_moves:
                    break

                # Mix of random and "reasonable" moves for diverse positions
                if random.random() < 0.15 and move_count < 10:
                    # Early game: sometimes random for diversity
                    move = random.choice(legal_moves)
                elif random.random() < 0.05:
                    # Occasional random move for variety
                    move = random.choice(legal_moves)
                else:
                    # Use Stockfish for a reasonable move (depth 1-4 for speed)
                    try:
                        result = engine.play(board, chess.engine.Limit(depth=random.randint(1, 4)))
                        move = result.move
                    except Exception:
                        move = random.choice(legal_moves)

                board.push(move)
                move_count += 1

                # Sample this position with some probability
                # Skip very early and very late positions
                if move_count >= 8 and random.random() < 0.35:
                    # Skip positions where game is decided
                    if not board.is_game_over():
                        positions_this_game.append(board.copy())

            # Evaluate sampled positions with deeper Stockfish
            for pos in positions_this_game:
                if written >= num_positions:
                    break
                try:
                    info = engine.analyse(pos, chess.engine.Limit(depth=depth))
                    score = info["score"].white()
                    if score.is_mate():
                        # Convert mate to high centipawn value
                        mate_in = score.mate()
                        if mate_in > 0:
                            cp = 3000 + (100 - min(mate_in, 100))
                        else:
                            cp = -3000 - (100 - min(abs(mate_in), 100))
                    else:
                        cp = score.score()

                    if cp is not None:
                        write_position(f, pos, cp)
                        written += 1

                        if written % batch_log == 0:
                            elapsed = time.time() - start_time
                            rate = written / elapsed
                            eta = (num_positions - written) / rate if rate > 0 else 0
                            print(f"[{written}/{num_positions}] "
                                  f"{rate:.0f} pos/s, "
                                  f"ETA: {eta/3600:.1f}h, "
                                  f"games: {games_played}")
                except Exception as e:
                    continue

            games_played += 1

    elapsed = time.time() - start_time
    print(f"\nDone! Generated {written} positions in {elapsed/3600:.2f}h "
          f"({games_played} games, {written/elapsed:.0f} pos/s)")


def generate_from_pgn(engine, pgn_path: str, num_positions: int,
                      output_path: str, depth: int = 10, batch_log: int = 10000):
    """Generate positions by sampling from a PGN file and evaluating with Stockfish."""
    written = 0
    games_read = 0
    start_time = time.time()

    with open(output_path, 'wb') as f_out, open(pgn_path, 'r') as f_pgn:
        while written < num_positions:
            game = chess.pgn.read_game(f_pgn)
            if game is None:
                print("Reached end of PGN, restarting from beginning...")
                f_pgn.seek(0)
                game = chess.pgn.read_game(f_pgn)
                if game is None:
                    break

            games_read += 1
            board = game.board()
            moves = list(game.mainline_moves())

            if len(moves) < 16:
                continue

            # Sample 1-5 random positions from this game
            num_samples = min(random.randint(1, 5), len(moves) - 8)
            sample_indices = sorted(random.sample(range(8, len(moves)), num_samples))

            for i, move in enumerate(moves):
                board.push(move)
                if i in sample_indices and not board.is_game_over():
                    try:
                        info = engine.analyse(board, chess.engine.Limit(depth=depth))
                        score = info["score"].white()
                        if score.is_mate():
                            mate_in = score.mate()
                            if mate_in > 0:
                                cp = 3000 + (100 - min(mate_in, 100))
                            else:
                                cp = -3000 - (100 - min(abs(mate_in), 100))
                        else:
                            cp = score.score()

                        if cp is not None:
                            write_position(f_out, board, cp)
                            written += 1

                            if written % batch_log == 0:
                                elapsed = time.time() - start_time
                                rate = written / elapsed
                                eta = (num_positions - written) / rate if rate > 0 else 0
                                print(f"[{written}/{num_positions}] "
                                      f"{rate:.0f} pos/s, "
                                      f"ETA: {eta/3600:.1f}h, "
                                      f"games: {games_read}")
                    except Exception:
                        continue

                if written >= num_positions:
                    break

    elapsed = time.time() - start_time
    print(f"\nDone! Generated {written} positions in {elapsed/3600:.2f}h "
          f"({games_read} games, {written/elapsed:.0f} pos/s)")


def main():
    parser = argparse.ArgumentParser(description='Generate NNUE training data')
    parser.add_argument('--stockfish', type=str, default='/usr/games/stockfish',
                        help='Path to Stockfish binary')
    parser.add_argument('--num-positions', type=int, default=5_000_000,
                        help='Number of positions to generate')
    parser.add_argument('--output', type=str, default='data.bin',
                        help='Output binary file')
    parser.add_argument('--depth', type=int, default=10,
                        help='Stockfish analysis depth')
    parser.add_argument('--pgn', type=str, default=None,
                        help='PGN file to sample positions from (optional)')
    parser.add_argument('--threads', type=int, default=2,
                        help='Stockfish threads')
    parser.add_argument('--hash', type=int, default=256,
                        help='Stockfish hash MB')

    args = parser.parse_args()

    print(f"Starting data generation:")
    print(f"  Stockfish: {args.stockfish}")
    print(f"  Depth: {args.depth}")
    print(f"  Target: {args.num_positions:,} positions")
    print(f"  Output: {args.output}")
    print(f"  Threads: {args.threads}, Hash: {args.hash}MB")

    engine = chess.engine.SimpleEngine.popen_uci(args.stockfish)
    engine.configure({"Threads": args.threads, "Hash": args.hash})

    try:
        if args.pgn:
            print(f"  Mode: PGN sampling from {args.pgn}")
            generate_from_pgn(engine, args.pgn, args.num_positions,
                              args.output, args.depth)
        else:
            print(f"  Mode: Random game generation")
            generate_random_positions(engine, args.num_positions,
                                     args.output, args.depth)
    finally:
        engine.quit()


if __name__ == '__main__':
    main()
