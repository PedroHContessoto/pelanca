"""
Pelanca NNUE - Network Architecture

Architecture:
    768 inputs -> Feature Transform (768->256, ClippedReLU) x2 perspectives
    -> Concat(stm_acc, nstm_acc) = 512
    -> Hidden (512->32, ClippedReLU)
    -> Output (32->1, linear)

The feature transform is shared between both perspectives.
Total parameters: ~205K
"""

import torch
import torch.nn as nn
import torch.nn.functional as F


# Constants matching Rust implementation
INPUT_SIZE = 768
FT_SIZE = 256      # Feature transform output
HIDDEN_SIZE = 32   # Hidden layer
OUTPUT_SIZE = 1

# Quantization scales (must match export and Rust inference)
FT_QUANT_SCALE = 64       # i16 weights for feature transform
HIDDEN_QUANT_SCALE = 64   # i8 weights for hidden layer
OUTPUT_QUANT_SCALE = 64   # i8 weights for output layer

# ClippedReLU clamp value (for quantized: 127)
CRELU_MAX = 1.0  # In float; becomes 127 in quantized


class ClippedReLU(nn.Module):
    """ReLU clamped to [0, max_val]. Becomes [0, 127] when quantized."""
    def __init__(self, max_val: float = CRELU_MAX):
        super().__init__()
        self.max_val = max_val

    def forward(self, x):
        return torch.clamp(x, 0.0, self.max_val)


class PelancaNNUE(nn.Module):
    """
    NNUE network for Pelanca chess engine.

    Forward pass:
    1. Compute feature transform for STM perspective: ft(stm_input) -> [256]
    2. Compute feature transform for NSTM perspective: ft(nstm_input) -> [256]
    3. Apply ClippedReLU to both
    4. Concatenate: [stm_acc, nstm_acc] -> [512]
    5. Hidden layer: 512 -> 32, ClippedReLU
    6. Output layer: 32 -> 1 (evaluation in centipawns / SCALE)
    """

    def __init__(self):
        super().__init__()

        # Feature transform (shared weights for both perspectives)
        self.ft = nn.Linear(INPUT_SIZE, FT_SIZE)

        # Hidden layer
        self.hidden = nn.Linear(FT_SIZE * 2, HIDDEN_SIZE)

        # Output layer
        self.output = nn.Linear(HIDDEN_SIZE, OUTPUT_SIZE)

        # Activations
        self.crelu = ClippedReLU()

        # Initialize weights
        self._init_weights()

    def _init_weights(self):
        # Kaiming initialization for ReLU layers
        nn.init.kaiming_normal_(self.ft.weight, nonlinearity='relu')
        nn.init.zeros_(self.ft.bias)
        nn.init.kaiming_normal_(self.hidden.weight, nonlinearity='relu')
        nn.init.zeros_(self.hidden.bias)
        # Output layer: small weights for stable start
        nn.init.xavier_normal_(self.output.weight)
        nn.init.zeros_(self.output.bias)

    def forward(self, stm_input, nstm_input):
        """
        Args:
            stm_input: [batch, 768] - features from side-to-move perspective
            nstm_input: [batch, 768] - features from non-side-to-move perspective

        Returns:
            [batch, 1] - evaluation from STM perspective (positive = good for STM)
        """
        # Feature transform (shared weights)
        stm_acc = self.crelu(self.ft(stm_input))    # [batch, 256]
        nstm_acc = self.crelu(self.ft(nstm_input))  # [batch, 256]

        # Concatenate perspectives
        combined = torch.cat([stm_acc, nstm_acc], dim=1)  # [batch, 512]

        # Hidden layer
        hidden = self.crelu(self.hidden(combined))  # [batch, 32]

        # Output (raw centipawns / scale)
        out = self.output(hidden)  # [batch, 1]

        return out

    def count_parameters(self):
        return sum(p.numel() for p in self.parameters())


# Loss function with sigmoid scaling
EVAL_SCALE = 400.0  # Centipawns per logistic unit


def scaled_mse_loss(pred, target):
    """MSE loss with sigmoid scaling.

    Focuses training on the tactically relevant range (-300 to +300 cp)
    rather than wasting capacity on extreme evaluations.

    Both pred and target are in centipawns.
    """
    pred_sigmoid = torch.sigmoid(pred / EVAL_SCALE)
    target_sigmoid = torch.sigmoid(target / EVAL_SCALE)
    return F.mse_loss(pred_sigmoid, target_sigmoid)
