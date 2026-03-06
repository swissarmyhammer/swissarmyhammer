#!/usr/bin/env -S uv run --python 3.12
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#     "torch>=2.7,<2.8",
#     "transformers>=4.51",
#     "coremltools>=8.0,<10.0",
#     "numpy",
# ]
# ///
"""Convert Qwen3-Embedding-0.6B to CoreML .mlpackage for Apple Neural Engine.

Uses torch.export (ExportedProgram) → coremltools conversion, which avoids
torch.jit.trace incompatibilities with modern transformers masking.

Supports post-training weight compression:
  --quantize palettize4  (4-bit palettization via k-means LUT — recommended)
  --quantize palettize2  (2-bit palettization)
  --quantize linear4     (4-bit linear symmetric quantization)
  --quantize linear8     (8-bit linear symmetric quantization)
  --quantize none        (fp16 weights, no compression — default)

Usage:
    uv run convert_qwen_embedding.py --quantize palettize4
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
        # Cast to float32 for coreml-rs compatibility (doesn't support f16 outputs yet)
        return (summed / counts).float()


def compress_model(mlmodel, quantize: str):
    """Apply post-training weight compression to the converted model."""
    if quantize == "none":
        return mlmodel

    if quantize.startswith("palettize"):
        nbits = int(quantize.replace("palettize", ""))
        print(f"Applying {nbits}-bit palettization (k-means LUT)...")
        op_config = cto.coreml.OpPalettizerConfig(
            mode="kmeans",
            nbits=nbits,
        )
        config = cto.coreml.OptimizationConfig(global_config=op_config)
        return cto.coreml.palettize_weights(mlmodel, config=config)

    elif quantize.startswith("linear"):
        nbits = int(quantize.replace("linear", ""))
        dtype_str = f"int{nbits}"
        print(f"Applying {nbits}-bit linear symmetric quantization...")
        op_config = cto.coreml.OpLinearQuantizerConfig(
            mode="linear_symmetric",
            dtype=dtype_str,
        )
        config = cto.coreml.OptimizationConfig(global_config=op_config)
        return cto.coreml.linear_quantize_weights(mlmodel, config=config)

    else:
        raise ValueError(f"Unknown quantize option: {quantize}")


def convert(output_dir: Path, seq_length: int = 512, quantize: str = "none"):
    model_name = "Qwen/Qwen3-Embedding-0.6B"

    print(f"Loading {model_name}...")
    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
    base_model = AutoModel.from_pretrained(
        model_name,
        trust_remote_code=True,
        torch_dtype=torch.float32,
    )
    base_model.eval()

    wrapper = EmbeddingWithPooling(base_model)
    wrapper.eval().float()

    # Verify model output before conversion
    dummy_ids = torch.ones(1, seq_length, dtype=torch.long)
    dummy_mask = torch.ones(1, seq_length, dtype=torch.long)
    with torch.no_grad():
        test_out = wrapper(dummy_ids, dummy_mask)
    hidden_dim = int(test_out.shape[-1])
    print(f"Model output shape: {test_out.shape} (hidden_dim={hidden_dim})")

    # Export using torch.export (avoids torch.jit.trace vmap issues)
    print(f"Exporting model with torch.export (static shape [1, {seq_length}])...")
    example_inputs = (dummy_ids, dummy_mask)
    exported = torch.export.export(wrapper, example_inputs)
    # Decompose high-level ops to ATEN dialect (required by coremltools)
    exported = exported.run_decompositions({})
    print("torch.export succeeded")

    # Convert ExportedProgram to CoreML
    print("Converting to CoreML mlprogram format (float16 compute, float32 output, CPU_AND_NE)...")
    mlmodel = ct.convert(
        exported,
        inputs=[
            ct.TensorType(name="input_ids", shape=(1, seq_length), dtype=np.int32),
            ct.TensorType(name="attention_mask", shape=(1, seq_length), dtype=np.int32),
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
        f"Qwen3-Embedding-0.6B with mean pooling ({quantize}). "
        f"Input: token IDs + attention mask [{seq_length}]. "
        f"Output: {hidden_dim}-dim embedding vector."
    )
    mlmodel.version = "1.0"

    output_path = output_dir / "Qwen3-Embedding-0.6B.mlpackage"
    if output_path.exists():
        shutil.rmtree(output_path)
    print(f"Saving to {output_path}...")
    mlmodel.save(str(output_path))

    total_size = sum(f.stat().st_size for f in output_path.rglob("*") if f.is_file())
    print(f"Saved .mlpackage ({total_size / 1e6:.1f} MB)")

    # Verify the saved model
    print("Verifying saved model...")
    loaded = ct.models.MLModel(str(output_path))
    test_input = {
        "input_ids": np.ones((1, seq_length), dtype=np.int32),
        "attention_mask": np.ones((1, seq_length), dtype=np.int32),
    }
    result = loaded.predict(test_input)
    embedding = result["embedding"]
    print(f"Verification output shape: {embedding.shape}")
    assert embedding.shape[-1] == hidden_dim, (
        f"Expected {hidden_dim}-dim, got {embedding.shape[-1]}"
    )
    print("Verification passed!")

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
        / "var" / "data" / "models" / "qwen3-embedding-0.6b"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=default_output,
        help=f"Output directory (default: {default_output})",
    )
    parser.add_argument(
        "--seq-length",
        type=int,
        default=512,
        help="Static sequence length (default: 512)",
    )
    parser.add_argument(
        "--quantize",
        choices=["none", "palettize2", "palettize4", "linear4", "linear8"],
        default="none",
        help="Post-training weight compression (default: none)",
    )
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    convert(args.output_dir, args.seq_length, args.quantize)


if __name__ == "__main__":
    main()
