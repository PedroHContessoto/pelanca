"""
Pelanca NNUE - Weight Export

Exports trained PyTorch model to compact binary format for Rust inference.

Binary format:
    Header:
        [4 bytes] magic: b'PLNN'
        [4 bytes] version: u32 = 1
        [4 bytes] input_size: u32 = 768
        [4 bytes] ft_size: u32 = 256
        [4 bytes] hidden_size: u32 = 32

    Feature Transform (768 -> 256):
        [256 * 768 * 2 bytes] ft_weight: i16[256][768] (row-major)
        [256 * 2 bytes] ft_bias: i16[256]

    Hidden Layer (512 -> 32):
        [32 * 512 bytes] hidden_weight: i8[32][512] (row-major)
        [32 * 4 bytes] hidden_bias: i32[32]

    Output Layer (32 -> 1):
        [32 bytes] output_weight: i8[1][32]
        [4 bytes] output_bias: i32[1]

    Total: 20 + 393,216 + 512 + 16,384 + 128 + 32 + 4 = ~410 KB
"""

import struct
import torch
import numpy as np
from model import PelancaNNUE, FT_QUANT_SCALE, HIDDEN_QUANT_SCALE, OUTPUT_QUANT_SCALE

MAGIC = b'PLNN'
VERSION = 1


def export_weights(model: PelancaNNUE, output_path: str, verbose: bool = True):
    """Export model weights to binary format for Rust inference."""
    model.eval()

    with open(output_path, 'wb') as f:
        # Header
        f.write(MAGIC)
        f.write(struct.pack('<I', VERSION))
        f.write(struct.pack('<I', 768))   # input_size
        f.write(struct.pack('<I', 256))   # ft_size
        f.write(struct.pack('<I', 32))    # hidden_size

        # Feature Transform weights (i16)
        ft_weight = model.ft.weight.data.cpu()  # [256, 768]
        ft_bias = model.ft.bias.data.cpu()      # [256]

        ft_w_q = (ft_weight * FT_QUANT_SCALE).round().clamp(-32767, 32767).to(torch.int16)
        ft_b_q = (ft_bias * FT_QUANT_SCALE).round().clamp(-32767, 32767).to(torch.int16)

        f.write(ft_w_q.numpy().tobytes())
        f.write(ft_b_q.numpy().tobytes())

        if verbose:
            print(f"FT weight: shape={ft_w_q.shape}, "
                  f"range=[{ft_w_q.min()}, {ft_w_q.max()}], "
                  f"bytes={ft_w_q.numpy().nbytes}")
            print(f"FT bias: shape={ft_b_q.shape}, "
                  f"range=[{ft_b_q.min()}, {ft_b_q.max()}], "
                  f"bytes={ft_b_q.numpy().nbytes}")

        # Hidden Layer weights (i8) and biases (i32)
        hidden_weight = model.hidden.weight.data.cpu()  # [32, 512]
        hidden_bias = model.hidden.bias.data.cpu()      # [32]

        h_w_q = (hidden_weight * HIDDEN_QUANT_SCALE).round().clamp(-127, 127).to(torch.int8)
        # Bias absorbs the FT scale: bias_q = bias * FT_SCALE * HIDDEN_SCALE
        h_b_q = (hidden_bias * FT_QUANT_SCALE * HIDDEN_QUANT_SCALE).round().clamp(
            -2**30, 2**30).to(torch.int32)

        f.write(h_w_q.numpy().tobytes())
        f.write(h_b_q.numpy().tobytes())

        if verbose:
            print(f"Hidden weight: shape={h_w_q.shape}, "
                  f"range=[{h_w_q.min()}, {h_w_q.max()}], "
                  f"bytes={h_w_q.numpy().nbytes}")
            print(f"Hidden bias: shape={h_b_q.shape}, "
                  f"range=[{h_b_q.min()}, {h_b_q.max()}], "
                  f"bytes={h_b_q.numpy().nbytes}")

        # Output Layer weights (i8) and bias (i32)
        output_weight = model.output.weight.data.cpu()  # [1, 32]
        output_bias = model.output.bias.data.cpu()      # [1]

        o_w_q = (output_weight * OUTPUT_QUANT_SCALE).round().clamp(-127, 127).to(torch.int8)
        # Output bias absorbs all scales
        o_b_q = (output_bias * FT_QUANT_SCALE * HIDDEN_QUANT_SCALE * OUTPUT_QUANT_SCALE).round().clamp(
            -2**30, 2**30).to(torch.int32)

        f.write(o_w_q.numpy().tobytes())
        f.write(o_b_q.numpy().tobytes())

        if verbose:
            print(f"Output weight: shape={o_w_q.shape}, "
                  f"range=[{o_w_q.min()}, {o_w_q.max()}], "
                  f"bytes={o_w_q.numpy().nbytes}")
            print(f"Output bias: shape={o_b_q.shape}, "
                  f"range=[{o_b_q.min()}, {o_b_q.max()}], "
                  f"bytes={o_b_q.numpy().nbytes}")

    import os
    total_size = os.path.getsize(output_path)
    if verbose:
        print(f"\nExported to {output_path} ({total_size:,} bytes / {total_size/1024:.1f} KB)")


def validate_export(model: PelancaNNUE, export_path: str, num_samples: int = 100):
    """Validate quantized weights against float model on random inputs."""
    import struct

    model.eval()

    # Load quantized weights
    with open(export_path, 'rb') as f:
        magic = f.read(4)
        assert magic == MAGIC, f"Bad magic: {magic}"
        version = struct.unpack('<I', f.read(4))[0]
        input_size = struct.unpack('<I', f.read(4))[0]
        ft_size = struct.unpack('<I', f.read(4))[0]
        hidden_size = struct.unpack('<I', f.read(4))[0]

        ft_w_data = np.frombuffer(f.read(ft_size * input_size * 2), dtype=np.int16).reshape(ft_size, input_size)
        ft_b_data = np.frombuffer(f.read(ft_size * 2), dtype=np.int16)
        h_w_data = np.frombuffer(f.read(hidden_size * ft_size * 2 * 1), dtype=np.int8).reshape(hidden_size, ft_size * 2)
        h_b_data = np.frombuffer(f.read(hidden_size * 4), dtype=np.int32)
        o_w_data = np.frombuffer(f.read(hidden_size * 1), dtype=np.int8).reshape(1, hidden_size)
        o_b_data = np.frombuffer(f.read(4), dtype=np.int32)

    print(f"\nValidation: comparing float vs quantized on {num_samples} random positions...")

    max_diff = 0
    total_diff = 0

    for _ in range(num_samples):
        # Create sparse random input (simulate ~32 pieces)
        stm_input = torch.zeros(1, 768)
        nstm_input = torch.zeros(1, 768)
        for _ in range(32):
            stm_input[0, torch.randint(768, (1,))] = 1.0
            nstm_input[0, torch.randint(768, (1,))] = 1.0

        # Float forward pass
        with torch.no_grad():
            float_out = model(stm_input, nstm_input).item()

        # Quantized forward pass (simulated)
        stm_indices = stm_input[0].nonzero().squeeze(-1).tolist()
        nstm_indices = nstm_input[0].nonzero().squeeze(-1).tolist()

        # Feature transform
        stm_acc = ft_b_data.copy().astype(np.int32)
        for idx in stm_indices:
            stm_acc += ft_w_data[:, idx].astype(np.int32)

        nstm_acc = ft_b_data.copy().astype(np.int32)
        for idx in nstm_indices:
            nstm_acc += ft_w_data[:, idx].astype(np.int32)

        # ClippedReLU (clamp to [0, FT_SCALE * CRELU_MAX] = [0, 64])
        stm_acc = np.clip(stm_acc, 0, FT_QUANT_SCALE)
        nstm_acc = np.clip(nstm_acc, 0, FT_QUANT_SCALE)

        # Concat
        combined = np.concatenate([stm_acc, nstm_acc]).astype(np.int8)

        # Hidden layer
        hidden = h_b_data.copy()
        for i in range(hidden_size):
            hidden[i] += np.sum(h_w_data[i].astype(np.int32) * combined.astype(np.int32))

        # ClippedReLU
        hidden_scale = FT_QUANT_SCALE * HIDDEN_QUANT_SCALE
        hidden = np.clip(hidden, 0, hidden_scale)

        # Output
        output = int(o_b_data[0])
        for i in range(hidden_size):
            output += int(o_w_data[0, i]) * int(hidden[i])

        # Descale
        total_scale = FT_QUANT_SCALE * HIDDEN_QUANT_SCALE * OUTPUT_QUANT_SCALE
        quant_out = output / total_scale

        diff = abs(float_out - quant_out)
        max_diff = max(max_diff, diff)
        total_diff += diff

    avg_diff = total_diff / num_samples
    print(f"  Avg diff: {avg_diff:.2f} cp")
    print(f"  Max diff: {max_diff:.2f} cp")
    print(f"  Status: {'OK' if max_diff < 50 else 'WARNING - large quantization error!'}")


if __name__ == '__main__':
    import sys

    if len(sys.argv) < 2:
        print("Usage: python export.py <model_checkpoint.pt> [output.nnue]")
        sys.exit(1)

    checkpoint_path = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) > 2 else 'pelanca.nnue'

    model = PelancaNNUE()
    checkpoint = torch.load(checkpoint_path, map_location='cpu')
    model.load_state_dict(checkpoint['model_state_dict'])

    export_weights(model, output_path)
    validate_export(model, output_path)
