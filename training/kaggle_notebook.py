#!/usr/bin/env python3
"""
Pelanca NNUE - Kaggle Training Notebook (script version)

Copie este conteudo para celulas de um notebook Kaggle:

Cell 1: Setup
Cell 2: Generate Data
Cell 3: Train
Cell 4: Export & Download
"""

# ============================================================================
# CELL 1: SETUP
# ============================================================================
# !pip install python-chess tqdm
# !apt-get install -y stockfish

import subprocess
import sys

def setup():
    subprocess.check_call([sys.executable, "-m", "pip", "install", "python-chess", "tqdm"])
    try:
        subprocess.check_call(["apt-get", "install", "-y", "stockfish"])
    except Exception:
        print("Stockfish install failed - may already be installed or need manual setup")

    # Verify
    import chess
    import torch
    print(f"python-chess: {chess.__version__}")
    print(f"PyTorch: {torch.__version__}")
    print(f"CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"GPU count: {torch.cuda.device_count()}")
        for i in range(torch.cuda.device_count()):
            print(f"  GPU {i}: {torch.cuda.get_device_name(i)}")


# ============================================================================
# CELL 2: GENERATE DATA
# ============================================================================

def generate_data(num_positions=5_000_000, depth=10):
    """Generate training data with Stockfish. ~2-4 hours for 5M positions."""
    from generate_data import main as gen_main
    sys.argv = [
        'generate_data.py',
        '--stockfish', '/usr/games/stockfish',
        '--num-positions', str(num_positions),
        '--output', 'data.bin',
        '--depth', str(depth),
        '--threads', '2',
        '--hash', '256',
    ]
    gen_main()


# ============================================================================
# CELL 3: TRAIN
# ============================================================================

def train(epochs=30, batch_size=16384):
    """Train NNUE. ~2-4 hours for 30 epochs on T4."""
    from train_nnue import main as train_main
    sys.argv = [
        'train_nnue.py',
        '--data', 'data.bin',
        '--epochs', str(epochs),
        '--batch-size', str(batch_size),
        '--output', 'pelanca.nnue',
        '--checkpoint-dir', 'checkpoints',
    ]
    train_main()


# ============================================================================
# CELL 4: EXPORT & DOWNLOAD
# ============================================================================

def export_and_validate():
    """Validate and show final results."""
    import os
    nnue_path = 'pelanca.nnue'
    if os.path.exists(nnue_path):
        size = os.path.getsize(nnue_path)
        print(f"NNUE file: {nnue_path} ({size:,} bytes / {size/1024:.1f} KB)")
        print("\nPara usar no Pelanca:")
        print("  1. Copie pelanca.nnue para a pasta nn/ do projeto")
        print("  2. Compile: cargo build --release")
        print("  3. O engine carregará o NNUE automaticamente")
    else:
        print("ERROR: pelanca.nnue not found!")


# ============================================================================
# MAIN - Run all steps
# ============================================================================

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--step', choices=['setup', 'generate', 'train', 'export', 'all'],
                        default='all')
    parser.add_argument('--num-positions', type=int, default=5_000_000)
    parser.add_argument('--depth', type=int, default=10)
    parser.add_argument('--epochs', type=int, default=30)
    args = parser.parse_args()

    if args.step in ('setup', 'all'):
        print("=== SETUP ===")
        setup()

    if args.step in ('generate', 'all'):
        print("\n=== GENERATE DATA ===")
        generate_data(args.num_positions, args.depth)

    if args.step in ('train', 'all'):
        print("\n=== TRAIN ===")
        train(args.epochs)

    if args.step in ('export', 'all'):
        print("\n=== EXPORT ===")
        export_and_validate()
