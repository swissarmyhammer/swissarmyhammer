#!/usr/bin/env -S uv run --python 3.12
# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = [
#     "torch>=2.7,<2.8",
#     "transformers>=4.51",
#     "coremltools>=8.0,<10.0",
#     "numpy",
#     "einops",
# ]
# ///
"""Convert jina-embeddings-v3 to CoreML .mlpackage for Apple Neural Engine.

Uses torch.export (ExportedProgram) → coremltools conversion with static shapes.
Merges the selected LoRA adapter into the base weights before export so the
CoreML model needs no adapter logic at runtime.

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


def merge_lora_weights(model, task: str):
    """Merge a LoRA adapter's weights into the base model in-place.

    jina-embeddings-v3 stores LoRA adapters as named parameters on
    each transformer layer. This finds lora_A/lora_B weight pairs
    for the selected task and merges them: W += (B @ A) * scale.
    """
    # The model exposes a set_adapter method if adapters are loaded
    if hasattr(model, "set_adapter"):
        print(f"  Using model.set_adapter('{task}')...")
        model.set_adapter(task)
        # After setting, we can try to merge
        if hasattr(model, "merge_adapter"):
            model.merge_adapter()
            print("  Merged adapter via model.merge_adapter()")
            return
        if hasattr(model, "merge_and_unload"):
            model.merge_and_unload()
            print("  Merged adapter via model.merge_and_unload()")
            return

    # Fallback: manually search for LoRA parameters
    merged_count = 0
    state_dict = dict(model.named_parameters())
    lora_a_keys = [k for k in state_dict if "lora_A" in k and task.replace(".", "_") in k]

    for a_key in lora_a_keys:
        b_key = a_key.replace("lora_A", "lora_B")
        if b_key not in state_dict:
            continue

        # Find the base weight key
        # Typical pattern: layer.attention.self.query.lora_A.retrieval_passage.weight
        # Base key: layer.attention.self.query.weight
        base_key = a_key.split(".lora_A")[0] + ".weight"
        if base_key not in state_dict:
            # Try without .weight suffix
            base_key = a_key.split(".lora_A")[0]
            if base_key not in state_dict:
                print(f"  Warning: no base weight for {a_key}")
                continue

        lora_a = state_dict[a_key].data
        lora_b = state_dict[b_key].data
        base = state_dict[base_key].data

        # LoRA merge: W = W + B @ A (scale is typically baked into weights)
        base.add_(lora_b @ lora_a)
        merged_count += 1

    if merged_count > 0:
        print(f"  Manually merged {merged_count} LoRA weight pairs for task '{task}'")
    else:
        print(f"  Warning: no LoRA weights found for task '{task}' — using base model")


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

    # Merge LoRA adapter into base weights
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

    # Export with torch.export
    print("Exporting model with torch.export...")
    example_inputs = (dummy_ids, dummy_mask)
    exported = torch.export.export(wrapper, example_inputs)
    exported = exported.run_decompositions({})
    print("torch.export succeeded")

    for sl in seq_lengths:
        print(f"\n--- Converting seq_len={sl} ---")

        output_path = output_dir / f"{OUTPUT_PREFIX}-seq{sl}.mlpackage"

        mlmodel = ct.convert(
            exported,
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
