#!/usr/bin/env python3
"""
Pelanca NNUE - Training Script

Designed to run on Kaggle with 2x T4 GPUs.
Can also run locally with a single GPU or CPU.

Usage (Kaggle):
    # Cell 1: Install dependencies
    !pip install python-chess tqdm

    # Cell 2: Generate data (or upload pre-generated)
    !python generate_data.py --stockfish /usr/games/stockfish --num-positions 5000000 --output data.bin

    # Cell 3: Train
    !python train_nnue.py --data data.bin --epochs 30 --batch-size 16384 --output pelanca.nnue

Usage (local):
    python train_nnue.py --data data.bin --epochs 30 --output pelanca.nnue
"""

import argparse
import os
import time

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import DataLoader, random_split
from torch.cuda.amp import GradScaler, autocast

from model import PelancaNNUE, scaled_mse_loss, EVAL_SCALE
from dataset import NnueDatasetV2
from export import export_weights, validate_export


def train_epoch(model, loader, optimizer, scaler, device, epoch, use_amp):
    model.train()
    total_loss = 0.0
    num_batches = 0
    start = time.time()

    for batch_idx, (stm_input, nstm_input, target) in enumerate(loader):
        stm_input = stm_input.to(device, non_blocking=True)
        nstm_input = nstm_input.to(device, non_blocking=True)
        target = target.to(device, non_blocking=True).unsqueeze(1)

        optimizer.zero_grad(set_to_none=True)

        if use_amp:
            with autocast():
                pred = model(stm_input, nstm_input)
                loss = scaled_mse_loss(pred, target)
            scaler.scale(loss).backward()
            scaler.unscale_(optimizer)
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            scaler.step(optimizer)
            scaler.update()
        else:
            pred = model(stm_input, nstm_input)
            loss = scaled_mse_loss(pred, target)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            optimizer.step()

        total_loss += loss.item()
        num_batches += 1

        if batch_idx % 100 == 0 and batch_idx > 0:
            avg_loss = total_loss / num_batches
            elapsed = time.time() - start
            samples_per_sec = (batch_idx + 1) * loader.batch_size / elapsed
            print(f"  Epoch {epoch} [{batch_idx}/{len(loader)}] "
                  f"loss={avg_loss:.6f} "
                  f"({samples_per_sec:.0f} samples/s)")

    return total_loss / max(num_batches, 1)


def validate(model, loader, device, use_amp):
    model.eval()
    total_loss = 0.0
    num_batches = 0

    with torch.no_grad():
        for stm_input, nstm_input, target in loader:
            stm_input = stm_input.to(device, non_blocking=True)
            nstm_input = nstm_input.to(device, non_blocking=True)
            target = target.to(device, non_blocking=True).unsqueeze(1)

            if use_amp:
                with autocast():
                    pred = model(stm_input, nstm_input)
                    loss = scaled_mse_loss(pred, target)
            else:
                pred = model(stm_input, nstm_input)
                loss = scaled_mse_loss(pred, target)

            total_loss += loss.item()
            num_batches += 1

    return total_loss / max(num_batches, 1)


def main():
    parser = argparse.ArgumentParser(description='Train Pelanca NNUE')
    parser.add_argument('--data', type=str, required=True, help='Training data binary file')
    parser.add_argument('--epochs', type=int, default=30, help='Number of epochs')
    parser.add_argument('--batch-size', type=int, default=16384, help='Batch size')
    parser.add_argument('--lr', type=float, default=1e-3, help='Learning rate')
    parser.add_argument('--weight-decay', type=float, default=1e-6, help='Weight decay')
    parser.add_argument('--output', type=str, default='pelanca.nnue', help='Output NNUE file')
    parser.add_argument('--checkpoint-dir', type=str, default='checkpoints', help='Checkpoint directory')
    parser.add_argument('--max-positions', type=int, default=None, help='Limit positions loaded')
    parser.add_argument('--val-split', type=float, default=0.02, help='Validation split ratio')
    parser.add_argument('--no-amp', action='store_true', help='Disable mixed precision')
    parser.add_argument('--resume', type=str, default=None, help='Resume from checkpoint')
    args = parser.parse_args()

    # Device setup
    if torch.cuda.is_available():
        device = torch.device('cuda')
        num_gpus = torch.cuda.device_count()
        print(f"Using {num_gpus} GPU(s): {[torch.cuda.get_device_name(i) for i in range(num_gpus)]}")
    else:
        device = torch.device('cpu')
        num_gpus = 0
        print("Using CPU")

    use_amp = torch.cuda.is_available() and not args.no_amp

    # Load data
    print("\n=== Loading Data ===")
    dataset = NnueDatasetV2(args.data, max_positions=args.max_positions)

    # Split into train/val
    val_size = int(len(dataset) * args.val_split)
    train_size = len(dataset) - val_size
    train_dataset, val_dataset = random_split(dataset, [train_size, val_size])
    print(f"Train: {train_size:,} positions, Val: {val_size:,} positions")

    # DataLoaders
    num_workers = min(4, os.cpu_count() or 1)
    train_loader = DataLoader(
        train_dataset,
        batch_size=args.batch_size,
        shuffle=True,
        num_workers=num_workers,
        pin_memory=torch.cuda.is_available(),
        drop_last=True,
    )
    val_loader = DataLoader(
        val_dataset,
        batch_size=args.batch_size,
        shuffle=False,
        num_workers=num_workers,
        pin_memory=torch.cuda.is_available(),
    )

    # Model
    print("\n=== Model ===")
    model = PelancaNNUE()
    print(f"Parameters: {model.count_parameters():,}")

    # Multi-GPU
    if num_gpus > 1:
        model = nn.DataParallel(model)
    model = model.to(device)

    # Optimizer & Scheduler
    optimizer = optim.Adam(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)
    scheduler = optim.lr_scheduler.ReduceLROnPlateau(optimizer, mode='min', factor=0.5,
                                                      patience=3, verbose=True)
    scaler = GradScaler() if use_amp else None

    start_epoch = 0
    best_val_loss = float('inf')

    # Resume from checkpoint
    if args.resume:
        print(f"Resuming from {args.resume}")
        checkpoint = torch.load(args.resume, map_location=device)
        model_to_load = model.module if isinstance(model, nn.DataParallel) else model
        model_to_load.load_state_dict(checkpoint['model_state_dict'])
        optimizer.load_state_dict(checkpoint['optimizer_state_dict'])
        start_epoch = checkpoint.get('epoch', 0) + 1
        best_val_loss = checkpoint.get('best_val_loss', float('inf'))

    # Checkpoints dir
    os.makedirs(args.checkpoint_dir, exist_ok=True)

    # Training loop
    print(f"\n=== Training ({args.epochs} epochs, batch={args.batch_size}, lr={args.lr}) ===")
    print(f"AMP: {'enabled' if use_amp else 'disabled'}")

    for epoch in range(start_epoch, args.epochs):
        epoch_start = time.time()

        # Train
        train_loss = train_epoch(model, train_loader, optimizer, scaler, device, epoch, use_amp)

        # Validate
        val_loss = validate(model, val_loader, device, use_amp)

        # LR scheduler
        scheduler.step(val_loss)

        elapsed = time.time() - epoch_start
        current_lr = optimizer.param_groups[0]['lr']
        print(f"Epoch {epoch}: train_loss={train_loss:.6f}, val_loss={val_loss:.6f}, "
              f"lr={current_lr:.2e}, time={elapsed:.1f}s")

        # Save checkpoint
        model_to_save = model.module if isinstance(model, nn.DataParallel) else model
        checkpoint = {
            'epoch': epoch,
            'model_state_dict': model_to_save.state_dict(),
            'optimizer_state_dict': optimizer.state_dict(),
            'train_loss': train_loss,
            'val_loss': val_loss,
            'best_val_loss': best_val_loss,
        }

        # Save latest
        torch.save(checkpoint, os.path.join(args.checkpoint_dir, 'latest.pt'))

        # Save best
        if val_loss < best_val_loss:
            best_val_loss = val_loss
            torch.save(checkpoint, os.path.join(args.checkpoint_dir, 'best.pt'))
            print(f"  -> New best model! val_loss={val_loss:.6f}")

        # Export NNUE every 5 epochs
        if (epoch + 1) % 5 == 0 or epoch == args.epochs - 1:
            nnue_path = os.path.join(args.checkpoint_dir, f'pelanca_epoch{epoch}.nnue')
            export_weights(model_to_save, nnue_path, verbose=False)
            print(f"  -> Exported {nnue_path}")

    # Final export from best model
    print("\n=== Exporting Best Model ===")
    best_checkpoint = torch.load(os.path.join(args.checkpoint_dir, 'best.pt'), map_location='cpu')
    final_model = PelancaNNUE()
    final_model.load_state_dict(best_checkpoint['model_state_dict'])
    export_weights(final_model, args.output)
    validate_export(final_model, args.output)

    print(f"\nDone! NNUE weights saved to {args.output}")
    print(f"Copy this file to your engine's nn/ directory.")


if __name__ == '__main__':
    main()
