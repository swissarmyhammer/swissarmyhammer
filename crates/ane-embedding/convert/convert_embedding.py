#!/usr/bin/env -S uv run --python 3.12
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#     "torch>=2.7,<2.8",
#     "transformers>=4.51,<4.52",
#     "coremltools>=8.0,<10.0",
#     "numpy",
#     "peft>=0.11",
#     "huggingface_hub>=0.20",
#     "scikit-learn",
# ]
# ///
"""Convert embedding models to CoreML .mlpackage for Apple Neural Engine.

Supports multiple models via a built-in registry. Uses torch.export
(ExportedProgram) -> coremltools conversion, which avoids torch.jit.trace
incompatibilities with modern transformers masking.

Produces a single model with EnumeratedShapes so the ANE can run at multiple
sequence lengths without padding waste.

Supports post-training weight compression:
  --compression palettize4  (4-bit palettization via k-means LUT -- recommended)
  --compression palettize2  (2-bit palettization)
  --compression linear4     (4-bit linear symmetric quantization)
  --compression linear8     (8-bit linear symmetric quantization)
  --compression none        (fp16 weights, no compression -- default)

Models:
  qwen3-embedding-0.6b       Qwen/Qwen3-Embedding-0.6B (dim=1024, mean pooling)
  nomic-embed-code            nomic-ai/nomic-embed-code (dim=3584, mean pooling, 7B — too large for ANE)

Usage:
    uv run convert_embedding.py --model qwen3-embedding-0.6b
    uv run convert_embedding.py --model all --compression palettize4
    uv run convert_embedding.py --model nomic-embed-code --upload
"""

import argparse
import shutil
from dataclasses import dataclass, field
from pathlib import Path

import coremltools as ct
import coremltools.optimize as cto
import numpy as np
import torch
import torch.nn as nn
from coremltools.converters.mil import Builder as mb
from coremltools.converters.mil import register_torch_op
from coremltools.converters.mil.frontend.torch.ops import _get_inputs
from transformers import AutoModel, AutoTokenizer


@register_torch_op
def new_ones(context, node):
    """Convert torch new_ones op to MIL (not natively supported by coremltools).

    new_ones(self, size, dtype, layout, device, pin_memory) -> Tensor
    """
    inputs = _get_inputs(context, node, expected=6)
    shape = inputs[1]
    # Cast shape to int32 if needed (mb.fill requires int32 shape)
    shape = mb.cast(x=shape, dtype="int32", name=node.name + "_shape")
    result = mb.fill(shape=shape, value=1.0, name=node.name)
    context.add(result)


@dataclass
class ModelSpec:
    """Specification for a supported embedding model."""

    repo: str
    dim: int
    seq_lengths: list[int] = field(default_factory=lambda: [64, 128, 256, 512])
    hf_upload: str = ""
    special_handling: str = ""
    pooling: str = "mean"  # "mean" or "last_token"


MODEL_REGISTRY: dict[str, ModelSpec] = {
    "qwen3-embedding-0.6b": ModelSpec(
        repo="Qwen/Qwen3-Embedding-0.6B",
        dim=1024,
        seq_lengths=[64, 128, 256, 512],
        hf_upload="wballard/Qwen3-Embedding-0.6B-CoreML",
    ),
    "nomic-embed-code": ModelSpec(
        repo="nomic-ai/nomic-embed-code",
        dim=3584,
        seq_lengths=[64, 128, 256, 512],
        hf_upload="wballard/nomic-embed-code-CoreML",
    ),
    "unixcoder-base": ModelSpec(
        repo="microsoft/unixcoder-base",
        dim=768,
        seq_lengths=[64, 128, 256, 512],
        hf_upload="wballard/unixcoder-base-CoreML",
    ),
}


class EmbeddingMeanPooling(nn.Module):
    """Wraps a transformer encoder to produce mean-pooled embeddings.

    Takes input_ids and attention_mask, runs through the encoder, and returns
    the mean-pooled last_hidden_state as a float32 embedding vector.
    """

    def __init__(self, encoder: nn.Module):
        super().__init__()
        self.encoder = encoder

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        outputs = self.encoder(input_ids=input_ids, attention_mask=attention_mask)
        hidden = outputs.last_hidden_state
        mask_expanded = attention_mask.unsqueeze(-1).float()
        summed = (hidden * mask_expanded).sum(dim=1)
        counts = mask_expanded.sum(dim=1).clamp(min=1e-9)
        return (summed / counts).float()


class EmbeddingLastTokenPooling(nn.Module):
    """Wraps a transformer encoder to produce last-token-pooled embeddings.

    Used by models like jina-code-embeddings-0.5b that pool from the last
    non-padding token rather than averaging all tokens.
    """

    def __init__(self, encoder: nn.Module):
        super().__init__()
        self.encoder = encoder

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        outputs = self.encoder(input_ids=input_ids, attention_mask=attention_mask)
        hidden = outputs.last_hidden_state
        # Find last non-padding token index per batch element
        seq_lens = attention_mask.sum(dim=1) - 1  # (batch,)
        last_hidden = hidden[torch.arange(hidden.size(0)), seq_lens]
        return last_hidden.float()


def compress_model(mlmodel, compression: str):
    """Apply post-training weight compression to the converted CoreML model.

    Args:
        mlmodel: A coremltools MLModel to compress.
        compression: Compression scheme name (palettize2, palettize4,
            linear4, linear8, or none).

    Returns:
        The compressed MLModel, or the original if compression is "none".

    Raises:
        ValueError: If the compression name is not recognized.
    """
    if compression == "none":
        return mlmodel

    if compression.startswith("palettize"):
        nbits = int(compression.replace("palettize", ""))
        print(f"Applying {nbits}-bit palettization (k-means LUT, per-channel scale)...")
        op_config = cto.coreml.OpPalettizerConfig(
            mode="kmeans",
            nbits=nbits,
            enable_per_channel_scale=True,
        )
        config = cto.coreml.OptimizationConfig(global_config=op_config)
        return cto.coreml.palettize_weights(mlmodel, config=config)

    if compression.startswith("linear"):
        nbits = int(compression.replace("linear", ""))
        dtype_str = f"int{nbits}"
        print(f"Applying {nbits}-bit linear symmetric quantization...")
        op_config = cto.coreml.OpLinearQuantizerConfig(
            mode="linear_symmetric",
            dtype=dtype_str,
        )
        config = cto.coreml.OptimizationConfig(global_config=op_config)
        return cto.coreml.linear_quantize_weights(mlmodel, config=config)

    raise ValueError(f"Unknown compression option: {compression}")


def load_model(spec: ModelSpec) -> nn.Module:
    """Load and prepare a HuggingFace model according to its spec.

    For models with special_handling="merge_lora", loads via peft and merges
    LoRA adapters before returning the base model.

    Args:
        spec: The ModelSpec describing which model to load and how.

    Returns:
        A torch.nn.Module ready for wrapping and export.
    """
    print(f"Loading {spec.repo}...")
    base_model = AutoModel.from_pretrained(
        spec.repo,
        trust_remote_code=True,
        torch_dtype=torch.float32,
        attn_implementation="eager",
    )

    if spec.special_handling == "merge_lora":
        print("Merging LoRA adapters...")
        base_model = base_model.merge_and_unload()

    base_model.eval()
    return base_model


def convert_model(model_key: str, spec: ModelSpec, output_dir: Path, compression: str, compute_precision: str = "float16"):
    """Convert a single embedding model to CoreML .mlpackage format.

    Performs the full pipeline: load model, wrap with mean pooling, export
    via torch.export, convert to CoreML with EnumeratedShapes, optionally
    compress weights, save .mlpackage, copy tokenizer, and verify output
    at each sequence length.

    Args:
        model_key: Registry key for the model (used in file naming).
        spec: ModelSpec with repo, dim, seq_lengths, etc.
        output_dir: Directory to write the .mlpackage and tokenizer into.
        compression: Weight compression scheme to apply after conversion.
    """
    seq_lengths = sorted(spec.seq_lengths)
    max_seq = max(seq_lengths)

    tokenizer = AutoTokenizer.from_pretrained(spec.repo, trust_remote_code=True)
    base_model = load_model(spec)

    if spec.pooling == "last_token":
        wrapper = EmbeddingLastTokenPooling(base_model)
    else:
        wrapper = EmbeddingMeanPooling(base_model)
    wrapper.eval().float()

    # Verify model output before conversion
    dummy_ids = torch.ones(1, max_seq, dtype=torch.long)
    dummy_mask = torch.ones(1, max_seq, dtype=torch.long)
    with torch.no_grad():
        test_out = wrapper(dummy_ids, dummy_mask)
    hidden_dim = int(test_out.shape[-1])
    print(f"Model output shape: {test_out.shape} (hidden_dim={hidden_dim})")

    if hidden_dim != spec.dim:
        print(f"WARNING: Expected dim={spec.dim} but model produced dim={hidden_dim}")

    # Export with dynamic sequence length dimension
    print(f"Exporting model with torch.export (dynamic seq_len, shapes={seq_lengths})...")
    example_inputs = (dummy_ids, dummy_mask)
    seq_dim = torch.export.Dim("seq_len", min=min(seq_lengths), max=max_seq)
    dynamic_shapes = {
        "input_ids": {1: seq_dim},
        "attention_mask": {1: seq_dim},
    }
    exported = torch.export.export(
        wrapper, example_inputs, dynamic_shapes=dynamic_shapes, strict=False
    )
    # Decompose to ATEN dialect (required by coremltools)
    exported = exported.run_decompositions({})
    print("torch.export succeeded")

    # Build EnumeratedShapes -- each length is a valid shape for ANE optimization
    shapes = [(1, sl) for sl in seq_lengths]
    default_shape = (1, max_seq)
    print(f"EnumeratedShapes: {shapes} (default={default_shape})")

    # Convert ExportedProgram to CoreML with enumerated shapes
    ct_precision = ct.precision.FLOAT16 if compute_precision == "float16" else ct.precision.FLOAT32
    print(f"Converting to CoreML mlprogram format ({compute_precision} compute, float32 output, CPU_AND_NE)...")
    mlmodel = ct.convert(
        exported,
        inputs=[
            ct.TensorType(
                name="input_ids",
                shape=ct.EnumeratedShapes(shapes=shapes, default=default_shape),
                dtype=np.int32,
            ),
            ct.TensorType(
                name="attention_mask",
                shape=ct.EnumeratedShapes(shapes=shapes, default=default_shape),
                dtype=np.int32,
            ),
        ],
        outputs=[ct.TensorType(name="embedding", dtype=np.float32)],
        convert_to="mlprogram",
        compute_precision=ct_precision,
        compute_units=ct.ComputeUnit.CPU_AND_NE,
        minimum_deployment_target=ct.target.macOS15,
    )

    # Apply post-training weight compression
    mlmodel = compress_model(mlmodel, compression)

    # Set metadata
    lengths_str = "/".join(str(s) for s in seq_lengths)
    repo_short = spec.repo.split("/")[-1]
    mlmodel.author = "swissarmyhammer"
    mlmodel.short_description = (
        f"{repo_short} with mean pooling ({compression}). "
        f"EnumeratedShapes: [{lengths_str}] tokens. "
        f"Output: {hidden_dim}-dim embedding vector."
    )
    mlmodel.version = "1.0"

    model_output_dir = output_dir / model_key
    model_output_dir.mkdir(parents=True, exist_ok=True)

    output_path = model_output_dir / f"{repo_short}.mlpackage"
    if output_path.exists():
        shutil.rmtree(output_path)
    print(f"Saving to {output_path}...")
    mlmodel.save(str(output_path))

    total_size = sum(f.stat().st_size for f in output_path.rglob("*") if f.is_file())
    print(f"Saved .mlpackage ({total_size / 1e6:.1f} MB)")

    # Verify at each sequence length
    loaded = ct.models.MLModel(str(output_path))
    for sl in seq_lengths:
        print(f"Verifying seq_len={sl}...")
        test_input = {
            "input_ids": np.ones((1, sl), dtype=np.int32),
            "attention_mask": np.ones((1, sl), dtype=np.int32),
        }
        result = loaded.predict(test_input)
        embedding = result["embedding"]
        assert embedding.shape[-1] == hidden_dim, (
            f"seq_len={sl}: Expected {hidden_dim}-dim, got {embedding.shape[-1]}"
        )
        print(f"  seq_len={sl}: OK ({embedding.shape})")

    print("All verifications passed!")

    # Save tokenizer alongside the model
    tokenizer_path = model_output_dir / "tokenizer.json"
    if not tokenizer_path.exists():
        tokenizer.save_pretrained(str(model_output_dir))
        print(f"Saved tokenizer to {model_output_dir}")

    print(f"Done converting {model_key}.")
    return model_output_dir


def upload_to_hub(model_key: str, spec: ModelSpec, folder_path: Path):
    """Upload a converted model folder to HuggingFace Hub.

    Args:
        model_key: Registry key for the model (used in log messages).
        spec: ModelSpec containing the hf_upload repo ID.
        folder_path: Local directory containing .mlpackage and tokenizer to upload.

    Raises:
        ValueError: If the model spec has no hf_upload repo configured.
    """
    if not spec.hf_upload:
        raise ValueError(f"Model {model_key!r} has no hf_upload repo configured")

    from huggingface_hub import HfApi

    api = HfApi()
    print(f"Uploading {folder_path} to {spec.hf_upload}...")
    api.upload_folder(
        folder_path=str(folder_path),
        repo_id=spec.hf_upload,
        repo_type="model",
    )
    print(f"Upload complete: https://huggingface.co/{spec.hf_upload}")


def main():
    """Entry point: parse arguments and run conversion for selected models."""
    valid_models = list(MODEL_REGISTRY.keys()) + ["all"]
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--model",
        required=True,
        choices=valid_models,
        help="Model to convert, or 'all' for every registered model.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("./output"),
        help="Output directory (default: ./output)",
    )
    parser.add_argument(
        "--compression",
        choices=["none", "palettize2", "palettize4", "linear4", "linear8"],
        default="palettize4",
        help="Post-training weight compression (default: palettize4)",
    )
    parser.add_argument(
        "--compute-precision",
        choices=["float16", "float32"],
        default="float16",
        help="CoreML compute precision (default: float16). Use float32 for models that produce NaN with float16.",
    )
    parser.add_argument(
        "--upload",
        action="store_true",
        help="Upload to HuggingFace Hub after conversion.",
    )
    args = parser.parse_args()

    if args.model == "all":
        models_to_convert = list(MODEL_REGISTRY.items())
    else:
        models_to_convert = [(args.model, MODEL_REGISTRY[args.model])]

    args.output_dir.mkdir(parents=True, exist_ok=True)

    for model_key, spec in models_to_convert:
        print(f"\n{'=' * 60}")
        print(f"Converting: {model_key} ({spec.repo})")
        print(f"{'=' * 60}\n")

        result_dir = convert_model(model_key, spec, args.output_dir, args.compression, args.compute_precision)

        if args.upload:
            upload_to_hub(model_key, spec, result_dir)

    print("\nAll requested conversions complete.")


if __name__ == "__main__":
    main()
