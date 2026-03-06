#!/usr/bin/env -S uv run --python 3.12
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#     "torch>=2.7,<2.8",
#     "transformers>=4.56",
#     "coremltools>=8.4,<10.0",
#     "numpy",
#     "scikit-learn",
# ]
# ///
"""Convert EmbeddingGemma-300M to CoreML .mlpackage for Apple Neural Engine.

EmbeddingGemma is a 308M parameter encoder-only model (Gemma3-based) that
produces 768-dim embeddings with mean pooling. Supports Matryoshka truncation
to 512/256/128 dims.

IMPORTANT: EmbeddingGemma activations do NOT support FP16 — they overflow
the FP16 range. This converter uses FP32 compute precision.

Uses torch.jit.trace → coremltools conversion with static shapes.

Usage:
    uv run convert_embeddinggemma.py
    uv run convert_embeddinggemma.py --quantize palettize4
    uv run convert_embeddinggemma.py --seq-lengths 128 256
"""

import argparse
import shutil
from pathlib import Path

import coremltools as ct
import coremltools.optimize as cto
import numpy as np
import torch
import torch.nn as nn
from transformers import AutoModel, AutoTokenizer

# Monkey-patch coremltools _cast to handle numpy arrays with shape (1,).
import coremltools.converters.mil.frontend.torch.ops as _ct_ops
from coremltools.converters.mil.frontend.torch.ops import (
    register_torch_op,
    _get_inputs,
)
from coremltools.converters.mil import Builder as mb

_orig_cast = _ct_ops._cast

def _patched_cast(context, node, dtype, dtype_name):
    try:
        return _orig_cast(context, node, dtype, dtype_name)
    except TypeError:
        inputs = _get_inputs(context, node, expected=1)
        x = inputs[0]
        if x.val is not None:
            val = x.val.item() if hasattr(x.val, 'item') else x.val
            res = mb.const(val=dtype(val), name=node.name)
            context.add(res, node.name)
        else:
            raise

_ct_ops._cast = _patched_cast

# Monkey-patch bitwise_and to handle float & bool (attention mask creation).
from coremltools.converters.mil.frontend.torch.torch_op_registry import _TORCH_OPS_REGISTRY

def _patched_bitwise_and(context, node):
    inputs = _get_inputs(context, node, expected=2)
    x, y = inputs[0], inputs[1]
    from coremltools.converters.mil.mil import types as _mil_types
    if x.dtype != _mil_types.bool:
        x = mb.cast(x=x, dtype="bool")
    if y.dtype != _mil_types.bool:
        y = mb.cast(x=y, dtype="bool")
    res = mb.logical_and(x=x, y=y, name=node.name)
    context.add(res, node.name)

_TORCH_OPS_REGISTRY.set_func_by_name(_patched_bitwise_and, "and")
_TORCH_OPS_REGISTRY.set_func_by_name(_patched_bitwise_and, "bitwise_and")

# Monkey-patch create_causal_mask to skip vmap-based mask generation.
# EmbeddingGemma uses bidirectional attention (encoder-only), so causal masks
# aren't needed. The vmap ops in masking_utils are incompatible with both
# torch.export and torch.jit.trace.
import transformers.masking_utils as _masking_utils

def _no_mask(**kwargs):
    """Return None to let the attention layer use its default (no mask)."""
    return None

_masking_utils.create_causal_mask = _no_mask
_masking_utils.create_sliding_window_causal_mask = _no_mask
# Also patch on the module that imports them
import transformers.models.gemma3.modeling_gemma3 as _gemma3_mod
_gemma3_mod.create_causal_mask = _no_mask
_gemma3_mod.create_sliding_window_causal_mask = _no_mask

MODEL_NAME = "google/embeddinggemma-300m"
OUTPUT_PREFIX = "EmbeddingGemma-300M"
DEFAULT_SEQ_LENGTHS = [128]


class EmbeddingWithPooling(nn.Module):
    """Wraps a transformer encoder to produce mean-pooled embeddings."""

    def __init__(self, encoder):
        super().__init__()
        self.encoder = encoder

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        outputs = self.encoder(input_ids=input_ids, attention_mask=attention_mask)
        hidden = outputs.last_hidden_state
        mask_expanded = attention_mask.unsqueeze(-1).float()
        summed = (hidden * mask_expanded).sum(dim=1)
        counts = mask_expanded.sum(dim=1).clamp(min=1e-9)
        return (summed / counts).float()


def compress_model(mlmodel, quantize: str):
    """Apply post-training weight compression to the converted model."""
    if quantize == "none":
        return mlmodel

    if quantize.startswith("palettize"):
        nbits = int(quantize.replace("palettize", ""))
        print(f"  Applying {nbits}-bit palettization (k-means LUT, per-channel scale)...")
        op_config = cto.coreml.OpPalettizerConfig(
            mode="kmeans",
            nbits=nbits,
            enable_per_channel_scale=True,
        )
        config = cto.coreml.OptimizationConfig(global_config=op_config)
        return cto.coreml.palettize_weights(mlmodel, config=config)

    elif quantize.startswith("linear"):
        nbits = int(quantize.replace("linear", ""))
        dtype_str = f"int{nbits}"
        print(f"  Applying {nbits}-bit linear symmetric quantization...")
        op_config = cto.coreml.OpLinearQuantizerConfig(
            mode="linear_symmetric",
            dtype=dtype_str,
        )
        config = cto.coreml.OptimizationConfig(global_config=op_config)
        return cto.coreml.linear_quantize_weights(mlmodel, config=config)

    else:
        raise ValueError(f"Unknown quantize option: {quantize}")


def convert(output_dir: Path, seq_lengths: list[int], quantize: str = "none"):
    max_seq = max(seq_lengths)

    print(f"Loading {MODEL_NAME}...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    # Use "eager" attention to avoid vmap-based SDPA masking, which is
    # incompatible with both torch.export and torch.jit.trace.
    base_model = AutoModel.from_pretrained(
        MODEL_NAME,
        torch_dtype=torch.float32,
        attn_implementation="eager",
    )
    base_model.eval()

    wrapper = EmbeddingWithPooling(base_model)
    wrapper.eval().float()

    # Verify model output before conversion
    dummy_ids = torch.ones(1, max_seq, dtype=torch.long)
    dummy_mask = torch.ones(1, max_seq, dtype=torch.long)
    with torch.no_grad():
        test_out = wrapper(dummy_ids, dummy_mask)
    hidden_dim = int(test_out.shape[-1])
    print(f"Model output shape: {test_out.shape} (hidden_dim={hidden_dim})")

    # Try torch.export first (cleanest for coremltools), fall back to trace
    try:
        print("Exporting model with torch.export...")
        example_inputs = (dummy_ids, dummy_mask)
        exported = torch.export.export(wrapper, example_inputs)
        exported = exported.run_decompositions({})
        print("torch.export succeeded")
        use_export = True
    except Exception as e:
        print(f"torch.export failed ({e}), falling back to torch.jit.trace...")
        with torch.no_grad():
            traced = torch.jit.trace(wrapper, (dummy_ids, dummy_mask))
        traced = torch.jit.freeze(traced)
        print("torch.jit.trace + freeze succeeded")
        use_export = False

    model_to_convert = exported if use_export else traced

    for sl in seq_lengths:
        print(f"\n--- Converting seq_len={sl} ---")

        output_path = output_dir / f"{OUTPUT_PREFIX}-seq{sl}.mlpackage"

        # Use FLOAT32 compute precision — EmbeddingGemma activations overflow FP16.
        mlmodel = ct.convert(
            model_to_convert,
            inputs=[
                ct.TensorType(name="input_ids", shape=(1, sl), dtype=np.int32),
                ct.TensorType(name="attention_mask", shape=(1, sl), dtype=np.int32),
            ],
            outputs=[ct.TensorType(name="embedding", dtype=np.float32)],
            convert_to="mlprogram",
            compute_precision=ct.precision.FLOAT32,
            compute_units=ct.ComputeUnit.CPU_AND_NE,
            minimum_deployment_target=ct.target.macOS15,
        )

        # Apply post-training weight compression
        mlmodel = compress_model(mlmodel, quantize)

        # Set metadata
        mlmodel.author = "swissarmyhammer"
        mlmodel.short_description = (
            f"EmbeddingGemma-300M with mean pooling ({quantize}). "
            f"Static shape: {sl} tokens. "
            f"Output: {hidden_dim}-dim embedding vector."
        )
        mlmodel.version = "1.0"

        if output_path.exists():
            shutil.rmtree(output_path)
        mlmodel.save(str(output_path))

        total_size = sum(f.stat().st_size for f in output_path.rglob("*") if f.is_file())
        print(f"  Saved {output_path.name} ({total_size / 1e6:.1f} MB)")

        # Verify
        loaded = ct.models.MLModel(str(output_path), compute_units=ct.ComputeUnit.CPU_ONLY)
        test_input = {
            "input_ids": np.ones((1, sl), dtype=np.int32),
            "attention_mask": np.ones((1, sl), dtype=np.int32),
        }
        result = loaded.predict(test_input)
        embedding = result["embedding"]
        assert embedding.shape[-1] == hidden_dim, (
            f"seq_len={sl}: Expected {hidden_dim}-dim, got {embedding.shape[-1]}"
        )
        assert not np.any(np.isnan(embedding)), (
            f"seq_len={sl}: Output contains NaN values"
        )
        assert np.abs(embedding).sum() > 0, (
            f"seq_len={sl}: Output is all zeros"
        )
        print(f"  Verified: shape={embedding.shape}, first_3={embedding.flatten()[:3]}")

    print(f"\nAll {len(seq_lengths)} models converted and verified!")

    # Save tokenizer alongside
    tokenizer_path = output_dir / "tokenizer.json"
    if not tokenizer_path.exists():
        tokenizer.save_pretrained(str(output_dir))
        print(f"Saved tokenizer to {output_dir}")

    print("Done.")


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    default_output = (
        Path(__file__).resolve().parent.parent.parent
        / "var" / "data" / "models" / "embeddinggemma-300m"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=default_output,
        help=f"Output directory (default: {default_output})",
    )
    parser.add_argument(
        "--seq-lengths",
        type=int,
        nargs="+",
        default=DEFAULT_SEQ_LENGTHS,
        help=f"Sequence lengths for static models (default: {DEFAULT_SEQ_LENGTHS})",
    )
    parser.add_argument(
        "--quantize",
        choices=["none", "palettize2", "palettize4", "linear4", "linear8"],
        default="none",
        help="Post-training weight compression (default: none)",
    )
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    args.seq_lengths.sort()
    convert(args.output_dir, args.seq_lengths, args.quantize)


if __name__ == "__main__":
    main()
