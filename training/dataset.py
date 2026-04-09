"""
Pelanca NNUE - Custom Dataset for binary training data.

Binary format per position:
    u8  side_to_move (0=White, 1=Black)
    u8  num_features
    u16 feature_indices[num_features]
    i16 score (centipawns, White's perspective)
"""

import struct
import torch
from torch.utils.data import Dataset
import numpy as np
from pathlib import Path


class NnueDataset(Dataset):
    """Memory-mapped dataset for NNUE training data."""

    def __init__(self, path: str, max_positions: int = None):
        self.path = path
        self.positions = []
        self._load(path, max_positions)

    def _load(self, path: str, max_positions: int = None):
        """Parse binary file into list of (stm, features, score) tuples."""
        file_size = Path(path).stat().st_size
        print(f"Loading {path} ({file_size / 1e6:.1f} MB)...")

        with open(path, 'rb') as f:
            data = f.read()

        offset = 0
        count = 0
        while offset < len(data):
            if max_positions and count >= max_positions:
                break

            stm = data[offset]
            num_features = data[offset + 1]
            offset += 2

            features = []
            for _ in range(num_features):
                idx = struct.unpack_from('<H', data, offset)[0]
                features.append(idx)
                offset += 2

            score = struct.unpack_from('<h', data, offset)[0]
            offset += 2

            self.positions.append((stm, features, score))
            count += 1

        print(f"Loaded {len(self.positions):,} positions")

    def __len__(self):
        return len(self.positions)

    def __getitem__(self, idx):
        stm, features, score = self.positions[idx]

        # Create sparse feature tensor (768 binary features)
        white_features = torch.zeros(384, dtype=torch.float32)
        black_features = torch.zeros(384, dtype=torch.float32)

        for feat_idx in features:
            color = feat_idx // 384  # 0=White, 1=Black
            remainder = feat_idx % 384
            if color == 0:
                white_features[remainder] = 1.0
            else:
                black_features[remainder] = 1.0

        # For Black's perspective: flip colors and mirror squares
        # Black sees white pieces as "opponent" and black pieces as "own"
        # Square mirroring: sq ^ 56 (vertical flip)
        white_perspective = torch.zeros(384, dtype=torch.float32)
        black_perspective = torch.zeros(384, dtype=torch.float32)

        for feat_idx in features:
            color = feat_idx // 384
            piece_type = (feat_idx % 384) // 64
            square = feat_idx % 64

            # White perspective: as-is
            wp_color = color
            wp_idx = wp_color * 384 + piece_type * 64 + square
            # Actually just use the feature directly since it's already in white perspective
            white_perspective[feat_idx % 384 if color == 0 else (feat_idx % 384)] = 1.0

            # Black perspective: flip color, mirror square
            bp_color = 1 - color  # flip
            bp_square = square ^ 56  # mirror
            bp_local = piece_type * 64 + bp_square
            black_perspective[bp_local] = 1.0

        # Reconstruct properly
        white_perspective = torch.zeros(384, dtype=torch.float32)
        black_perspective = torch.zeros(384, dtype=torch.float32)

        for feat_idx in features:
            color = feat_idx // 384
            piece_type = (feat_idx % 384) // 64
            square = feat_idx % 64

            # White perspective: feature as stored
            w_local = piece_type * 64 + square
            if color == 0:
                # White piece in white perspective = "own" piece
                white_perspective[w_local] = 1.0
            else:
                # Black piece in white perspective = "opponent" piece
                white_perspective[192 + w_local] = 1.0  # offset by 192 (3 piece types * 64)

            # Black perspective: flip color, mirror square
            b_square = square ^ 56
            b_local = piece_type * 64 + b_square
            if color == 1:
                # Black piece in black perspective = "own" piece
                black_perspective[b_local] = 1.0
            else:
                # White piece in black perspective = "opponent" piece
                black_perspective[192 + b_local] = 1.0

        # Actually, let me simplify: use the raw 768 features for both perspectives
        # The perspective handling will be done differently

        # Simple approach: just use the 768 features directly
        # Perspective is handled by the model (concat order based on STM)
        features_tensor = torch.zeros(768, dtype=torch.float32)
        for feat_idx in features:
            features_tensor[feat_idx] = 1.0

        score_tensor = torch.tensor(score, dtype=torch.float32)
        stm_tensor = torch.tensor(stm, dtype=torch.float32)

        return features_tensor, stm_tensor, score_tensor


class NnueDatasetV2(Dataset):
    """Optimized dataset with perspective-relative features pre-computed.

    Each sample returns:
        stm_features: [384] - features from side-to-move's perspective
        nstm_features: [384] - features from non-side-to-move's perspective
        score: scalar - evaluation in centipawns (from White's perspective)
        stm: scalar - 0=White, 1=Black
    """

    def __init__(self, path: str, max_positions: int = None):
        self.path = path
        # Store as numpy arrays for memory efficiency
        self.stm_array = []
        self.score_array = []
        self.features_list = []  # list of feature index lists
        self._load(path, max_positions)

    def _load(self, path: str, max_positions: int = None):
        file_size = Path(path).stat().st_size
        print(f"Loading {path} ({file_size / 1e6:.1f} MB)...")

        with open(path, 'rb') as f:
            data = f.read()

        offset = 0
        count = 0
        while offset < len(data):
            if max_positions and count >= max_positions:
                break

            stm = data[offset]
            num_features = data[offset + 1]
            offset += 2

            features = []
            for _ in range(num_features):
                idx = struct.unpack_from('<H', data, offset)[0]
                features.append(idx)
                offset += 2

            score = struct.unpack_from('<h', data, offset)[0]
            offset += 2

            self.stm_array.append(stm)
            self.score_array.append(score)
            self.features_list.append(features)
            count += 1

        self.stm_array = np.array(self.stm_array, dtype=np.uint8)
        self.score_array = np.array(self.score_array, dtype=np.int16)
        print(f"Loaded {len(self.features_list):,} positions")

    def __len__(self):
        return len(self.features_list)

    def __getitem__(self, idx):
        stm = self.stm_array[idx]
        score = self.score_array[idx]
        features = self.features_list[idx]

        # Build perspective-relative feature vectors
        # Each perspective sees: own_pieces (6 types * 64 sq) + opp_pieces (6 types * 64 sq) = 768
        # But we use the shared feature transform, so we compute
        # white_acc and black_acc separately using 384 features each

        # White accumulator input: all pieces from White's viewpoint
        white_input = torch.zeros(768, dtype=torch.float32)
        black_input = torch.zeros(768, dtype=torch.float32)

        for feat_idx in features:
            color = feat_idx // 384  # 0=White piece, 1=Black piece
            piece_type = (feat_idx % 384) // 64
            square = feat_idx % 64

            # White perspective: pieces as-is
            white_input[feat_idx] = 1.0

            # Black perspective: flip color (0->1, 1->0) and mirror square (sq^56)
            flipped_color = 1 - color
            mirrored_sq = square ^ 56
            black_feat = flipped_color * 384 + piece_type * 64 + mirrored_sq
            black_input[black_feat] = 1.0

        # Score from White's perspective -> convert to STM perspective for training
        # The model outputs from STM perspective, so target should be STM-relative
        if stm == 1:  # Black to move
            stm_score = -score
        else:
            stm_score = score

        # Return based on side to move
        if stm == 0:  # White to move
            stm_input = white_input
            nstm_input = black_input
        else:  # Black to move
            stm_input = black_input
            nstm_input = white_input

        return (stm_input, nstm_input,
                torch.tensor(stm_score, dtype=torch.float32))
