#!/usr/bin/env -S uv run --python 3.12
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#     "torch>=2.7,<2.8",
#     "transformers>=4.45,<4.51",
#     "coremltools>=8.0,<10.0",
#     "numpy",
#     "einops",
# ]
# ///
"""Convert jina-embeddings-v3 to CoreML .mlpackage for Apple Neural Engine.

Merges the selected LoRA adapter into the base weights, then exports via
torch.export (ExportedProgram) → coremltools conversion with static shapes.

LoRA weights are explicitly merged (base + B@A * scaling) and parametrizations
removed before export so the CoreML model needs no adapter logic at runtime.

Produces one .mlpackage per sequence length (default: 128).

Usage:
    uv run convert_jina_embedding.py
    uv run convert_jina_embedding.py --task retrieval.passage
    uv run convert_jina_embedding.py --seq-lengths 128 256
"""

import argparse
import shutil
from pathlib import Path

import coremltools as ct
import numpy as np
import torch
import torch.nn as nn
from transformers import AutoModel, AutoTokenizer

# Monkey-patch coremltools _cast to handle numpy arrays with shape (1,).
# jina's einops.rearrange produces traced `int` cast ops on shape values
# that are 1-element arrays, and coremltools tries `int(np.array([128]))`
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

DEFAULT_SEQ_LENGTHS = [128]
MODEL_NAME = "jinaai/jina-embeddings-v3"
OUTPUT_PREFIX = "jina-embeddings-v3"

# Available LoRA tasks in jina-embeddings-v3
LORA_TASKS = [
    "retrieval.query",
    "retrieval.passage",
    "separation",
    "classification",
    "text-matching",
]


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


def merge_lora_weights(model, task: str):
    """Merge LoRA adapter weights for a specific task into the base model.

    jina-embeddings-v3 uses torch.nn.utils.parametrize to register LoRA
    parametrizations AND monkey-patches each layer's forward method. The
    LoRA is only applied when task_id is passed; without it, the model
    uses base weights (LoRAParametrization.forward is identity).

    To bake the LoRA for a specific task:
    1. Compute merged_weight = base + (B[task_idx] @ A[task_idx]) * scaling
    2. Remove the parametrization
    3. Set the weight to the merged value
    4. Restore the original forward (removing the monkey-patched version)
    """
    import torch.nn.utils.parametrize as parametrize

    # Get task index from model's adaptation map
    task_idx = model._adaptation_map[task]
    print(f"  Task '{task}' → index {task_idx}")

    merged_count = 0
    for name, module in list(model.named_modules()):
        if not parametrize.is_parametrized(module, "weight"):
            continue

        lora_param = module.parametrizations.weight[0]
        lora_A = lora_param.lora_A[task_idx].data  # (rank, fan_in) or (fan_in, rank)
        lora_B = lora_param.lora_B[task_idx].data  # (fan_out, rank) or (rank, fan_out)
        scaling = lora_param.scaling

        # Get the base weight before removing parametrization
        base_weight = module.parametrizations.weight.original.data.clone()

        # Compute LoRA delta: swap handles linear vs embedding layout
        delta = torch.matmul(*lora_param.swap((lora_B, lora_A))) * scaling

        # Merge
        merged_weight = base_weight + delta.view_as(base_weight)

        # Determine original forward to restore
        is_embedding = isinstance(module, nn.Embedding)
        # Check if this was a LinearResidual before parametrization
        is_linear_residual = hasattr(module, '__class__') and any(
            'LinearResidual' in c.__name__
            for c in type(module).__mro__
            if hasattr(c, '__name__')
        )

        # Remove parametrization (restores original class)
        parametrize.remove_parametrizations(module, "weight", leave_parametrized=False)

        # Set merged weight
        module.weight.data.copy_(merged_weight)

        # Restore original forward (remove monkey-patched new_forward)
        if is_embedding:
            module.forward = nn.Embedding.forward.__get__(module, type(module))
        elif is_linear_residual:
            # LinearResidual.forward returns (output, input) — needed by mha.py
            from types import MethodType
            def _linear_residual_forward(self, input):
                return nn.functional.linear(input, self.weight, self.bias), input
            module.forward = MethodType(_linear_residual_forward, module)
        else:
            module.forward = nn.Linear.forward.__get__(module, type(module))

        merged_count += 1

    print(f"  Merged {merged_count} LoRA weights for task '{task}'")


def convert(output_dir: Path, seq_lengths: list[int], task: str):
    max_seq = max(seq_lengths)

    print(f"Loading {MODEL_NAME} (task={task})...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME, trust_remote_code=True)
    base_model = AutoModel.from_pretrained(
        MODEL_NAME,
        trust_remote_code=True,
        torch_dtype=torch.float32,
    )
    base_model.eval()

    # Merge LoRA adapter weights into base model for the selected task
    print(f"Merging LoRA adapter '{task}'...")
    merge_lora_weights(base_model, task)

    wrapper = EmbeddingWithPooling(base_model)
    wrapper.eval().float()

    # Verify model output before conversion
    dummy_ids = torch.ones(1, max_seq, dtype=torch.long)
    dummy_mask = torch.ones(1, max_seq, dtype=torch.long)
    with torch.no_grad():
        test_out = wrapper(dummy_ids, dummy_mask)
    hidden_dim = int(test_out.shape[-1])
    print(f"Model output shape: {test_out.shape} (hidden_dim={hidden_dim})")

    # Trace with torch.jit.trace. After LoRA merge, parametrizations are
    # removed and forwards restored, so the computation graph is clean.
    #
    # Note: coremltools can't handle:
    # - torch.export ExportedProgram: unsupported `alias` EXIR nodes
    # - torch.jit.trace with einops: `int` cast ops from rearrange
    #
    # Fix: monkey-patch einops.rearrange to use torch.reshape before tracing.
    # This eliminates the dynamic shape ops that coremltools can't convert.
    import einops
    _orig_rearrange = einops.rearrange

    def _static_rearrange(tensor, pattern, **axes_lengths):
        """Replacement for einops.rearrange that uses reshape for simple cases."""
        return _orig_rearrange(tensor, pattern, **axes_lengths)

    # Actually, the issue is deeper — einops itself is fine, the problem is
    # that the traced graph contains aten::Int nodes from shape computations.
    # Instead, let's use torch.jit.trace and pass through coremltools with
    # the `pass_pipeline` to skip problematic ops.
    print("Tracing model with torch.jit.trace...")
    with torch.no_grad():
        traced = torch.jit.trace(wrapper, (dummy_ids, dummy_mask))
    # Freeze the traced model to fold constants and eliminate dynamic shape ops
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

        # Set metadata
        mlmodel.author = "swissarmyhammer"
        mlmodel.short_description = (
            f"jina-embeddings-v3 with mean pooling (task={task}). "
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
        loaded = ct.models.MLModel(str(output_path))
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
        / "var" / "data" / "models" / "jina-embeddings-v3"
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
        "--task",
        choices=LORA_TASKS,
        default="text-matching",
        help="LoRA task adapter to merge (default: text-matching)",
    )
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    args.seq_lengths.sort()
    convert(args.output_dir, args.seq_lengths, args.task)


if __name__ == "__main__":
    main()
