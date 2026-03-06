#!/usr/bin/env -S uv run --python 3.12
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#     "torch>=2.7,<2.8",
#     "transformers>=4.51",
#     "coremltools>=8.0,<10.0",
#     "numpy",
#     "scikit-learn",
# ]
# ///
"""Convert nomic-embed-code to CoreML .mlpackage for Apple Neural Engine.

nomic-embed-code is a 7B Qwen2.5-Coder-based code embedding model.
At 7B parameters the FP16 model is ~14GB — use --quantize palettize4
(default) to compress to ~3.5GB for practical on-device use.

Uses torch.export (ExportedProgram) → coremltools conversion with static shapes.

Usage:
    uv run convert_nomic_embedding.py
    uv run convert_nomic_embedding.py --quantize none
    uv run convert_nomic_embedding.py --seq-lengths 128 256
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
# Qwen2.5's attention mask code produces traced `int` cast ops on shape
# values that are 1-element arrays. coremltools tries `int(np.array([128]))`
# which raises "only 0-dimensional arrays can be converted to Python scalars".
import coremltools.converters.mil.frontend.torch.ops as _ct_ops

_orig_cast = _ct_ops._cast

def _patched_cast(context, node, dtype, dtype_name):
    try:
        return _orig_cast(context, node, dtype, dtype_name)
    except TypeError:
        from coremltools.converters.mil.frontend.torch.ops import _get_inputs
        from coremltools.converters.mil import Builder as mb
        inputs = _get_inputs(context, node, expected=1)
        x = inputs[0]
        if x.val is not None:
            val = x.val.item() if hasattr(x.val, 'item') else x.val
            res = mb.const(val=dtype(val), name=node.name)
            context.add(res, node.name)
        else:
            raise

_ct_ops._cast = _patched_cast

# Register converter for `new_ones` op — Qwen2.5 uses tensor.new_ones()
# for causal mask creation, which coremltools doesn't support natively.
from coremltools.converters.mil.frontend.torch.ops import (
    register_torch_op,
    _get_inputs,
)
from coremltools.converters.mil import Builder as mb

# Monkey-patch bitwise_and to handle float & bool (attention mask creation).
# coremltools' built-in bitwise_and rejects non-boolean inputs — we cast to bool first.
# sanitize_op_kind transforms "__and__" → "and", so we must patch that registry key.
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

@register_torch_op
def new_ones(context, node):
    inputs = _get_inputs(context, node)
    # new_ones(self, size, dtype=None, layout=None, device=None, pin_memory=False)
    # inputs[0] = self tensor, inputs[1] = size
    # Size may be a list of MIL vars or a single var with shape info
    size = inputs[1]
    if hasattr(size, 'val') and size.val is not None:
        shape_val = [int(x) for x in np.atleast_1d(size.val)]
    elif isinstance(size, (list, tuple)):
        shape_val = [int(s.val) if hasattr(s, 'val') and s.val is not None else s for s in size]
    else:
        # Try to get shape from the node directly
        shape_val = list(size.shape) if hasattr(size, 'shape') else [1]
    res = mb.fill(shape=np.array(shape_val, dtype=np.int32), value=1.0, name=node.name)
    context.add(res, node.name)

MODEL_NAME = "nomic-ai/nomic-embed-code"
OUTPUT_PREFIX = "nomic-embed-code"
DEFAULT_SEQ_LENGTHS = [128]


class EmbeddingWithPooling(nn.Module):
    """Wraps a transformer encoder to produce last-token-pooled embeddings.

    nomic-embed-code uses last-token pooling (not mean pooling).
    The last non-padding token's hidden state is the embedding.
    """

    def __init__(self, encoder):
        super().__init__()
        self.encoder = encoder

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        outputs = self.encoder(input_ids=input_ids, attention_mask=attention_mask)
        hidden = outputs.last_hidden_state
        # Last-token pooling: find last non-padding position per sequence
        # For static shapes with tracing, use sum of attention_mask - 1
        seq_lengths = attention_mask.sum(dim=1, keepdim=True) - 1  # (batch, 1)
        seq_lengths = seq_lengths.clamp(min=0)
        # Gather the hidden state at the last token position
        indices = seq_lengths.unsqueeze(-1).expand(-1, -1, hidden.shape[-1])  # (batch, 1, hidden)
        pooled = hidden.gather(1, indices.long()).squeeze(1)  # (batch, hidden)
        return pooled.float()


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


def convert(output_dir: Path, seq_lengths: list[int], quantize: str = "palettize4"):
    max_seq = max(seq_lengths)

    print(f"Loading {MODEL_NAME}...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME, trust_remote_code=True)
    base_model = AutoModel.from_pretrained(
        MODEL_NAME,
        trust_remote_code=True,
        torch_dtype=torch.float32,
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

    # Trace with torch.jit.trace. torch.export produces `new_ones` EXIR
    # nodes from Qwen2.5's causal mask creation that coremltools can't handle.
    print("Tracing model with torch.jit.trace...")
    with torch.no_grad():
        traced = torch.jit.trace(wrapper, (dummy_ids, dummy_mask))
    traced = torch.jit.freeze(traced)
    print("torch.jit.trace + freeze succeeded")

    for sl in seq_lengths:
        print(f"\n--- Converting seq_len={sl} ---")

        output_path = output_dir / f"{OUTPUT_PREFIX}-seq{sl}.mlpackage"

        mlmodel = ct.convert(
            traced,
            inputs=[
                ct.TensorType(name="input_ids", shape=(1, sl), dtype=np.int32),
                ct.TensorType(name="attention_mask", shape=(1, sl), dtype=np.int32),
            ],
            outputs=[ct.TensorType(name="embedding", dtype=np.float32)],
            convert_to="mlprogram",
            compute_precision=ct.precision.FLOAT16,
            compute_units=ct.ComputeUnit.CPU_AND_NE,
            minimum_deployment_target=ct.target.macOS15,
        )

        # Apply post-training weight compression
        mlmodel = compress_model(mlmodel, quantize)

        # Set metadata
        mlmodel.author = "swissarmyhammer"
        mlmodel.short_description = (
            f"nomic-embed-code with last-token pooling ({quantize}). "
            f"Static shape: {sl} tokens. "
            f"Output: {hidden_dim}-dim embedding vector."
        )
        mlmodel.version = "1.0"

        if output_path.exists():
            shutil.rmtree(output_path)
        mlmodel.save(str(output_path))

        total_size = sum(f.stat().st_size for f in output_path.rglob("*") if f.is_file())
        print(f"  Saved {output_path.name} ({total_size / 1e6:.1f} MB)")

        # Verify — use CPU_ONLY to avoid MPS dequantize assertion failures
        # with palettized embedding tables. ANE handles them fine at runtime.
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
        / "var" / "data" / "models" / "nomic-embed-code"
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
        default="palettize4",
        help="Post-training weight compression (default: palettize4)",
    )
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    args.seq_lengths.sort()
    convert(args.output_dir, args.seq_lengths, args.quantize)


if __name__ == "__main__":
    main()
