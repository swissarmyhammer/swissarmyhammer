# CLI Usage Examples

This document provides two working examples of using the llama-cli for text generation.

## Example 1: HuggingFace Model

Generate text using a HuggingFace model with basic parameters:

```bash
llama-cli generate \
  --model unsloth/Qwen3-4B-GGUF \
  --filename Qwen3-4B-UD-Q4_K_XL.gguf \
  --prompt "What is the capital of France?" \
  --limit 100 \
  --temperature 0.7
```

This command:
- Downloads and uses the Qwen2.5-0.5B-Instruct-GGUF model from HuggingFace
- Generates up to 100 tokens
- Uses a temperature of 0.7 for balanced creativity
- Auto-detects the best model file (prefers BF16 format)

## Example 2: Local Model

Generate text using a local model folder:

```bash
llama-cli generate \
  --model ./models/my-model \
  --prompt "Explain how neural networks work" \
  --limit 200 \
  --temperature 0.3 \
  --filename model.gguf
```

This command:
- Uses a local model stored in `./models/my-model/`
- Specifically loads the `model.gguf` file
- Generates up to 200 tokens
- Uses lower temperature (0.3) for more focused responses
