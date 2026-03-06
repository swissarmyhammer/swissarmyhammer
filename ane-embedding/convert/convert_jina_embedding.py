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

Uses torch.jit.trace → coremltools conversion with static shapes.
(torch.export produces `alias` fx nodes that coremltools cannot convert
for jina's custom XLM-RoBERTa architecture.)

LoRA parametrizations are kept intact during tracing — torch.jit.trace
captures the full merged computation graph, so the CoreML model needs
no adapter logic at runtime.

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


def select_lora_task(model, task: str):
    """Select and activate the LoRA adapter for the given task.

    jina-embeddings-v3 uses PyTorch parametrizations to store LoRA weights.
    The parametrized weights compute merged (base + LoRA) values on-the-fly.
    We do NOT remove parametrizations — jina's attention code checks
    hasattr(Wqkv, 'parametrizations') to decide the forward path, and
    removing them breaks that control flow.

    torch.jit.trace will capture the full computation graph including the
    parametrized weight merge, which is exactly what we want.
    """
    import torch.nn.utils.parametrize as parametrize

    param_count = sum(
        1 for _, m in model.named_modules()
        if parametrize.is_parametrized(m, "weight")
    )
    if param_count > 0:
        print(f"  Found {param_count} parametrized LoRA weights (kept intact for tracing)")
    else:
        print(f"  No parametrized weights found — using model as-is")


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

    # Select LoRA task — parametrizations are kept intact for tracing
    print(f"Selecting LoRA adapter '{task}'...")
    select_lora_task(base_model, task)

    wrapper = EmbeddingWithPooling(base_model)
    wrapper.eval().float()

    # Verify model output before conversion
    dummy_ids = torch.ones(1, max_seq, dtype=torch.long)
    dummy_mask = torch.ones(1, max_seq, dtype=torch.long)
    with torch.no_grad():
        test_out = wrapper(dummy_ids, dummy_mask)
    hidden_dim = int(test_out.shape[-1])
    print(f"Model output shape: {test_out.shape} (hidden_dim={hidden_dim})")

    # Trace with torch.jit.trace (torch.export produces `alias` fx nodes
    # that coremltools cannot convert for jina's XLM-RoBERTa architecture)
    print("Tracing model with torch.jit.trace...")
    example_inputs = (dummy_ids, dummy_mask)
    traced = torch.jit.trace(wrapper, example_inputs)
    print("torch.jit.trace succeeded")

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
