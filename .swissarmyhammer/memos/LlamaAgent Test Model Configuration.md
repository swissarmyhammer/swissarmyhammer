# LlamaAgent Test Model Configuration

## Recommended Test Model

For testing llama-agent integration with SwissArmyHammer, use this lightweight model:

**Repository:** `unsloth/Qwen3-1.7B-GGUF`
**Filename:** `Qwen3-1.7B-UD-Q6_K_XL.gguf`

## Configuration Example

```yaml
model:
  source:
    HuggingFace:
      repo: "unsloth/Qwen3-1.7B-GGUF"
      filename: "Qwen3-1.7B-UD-Q6_K_XL.gguf"
  batch_size: 256
  use_hf_params: true
  debug: true
```

## Why This Model?

- **Small Size**: ~1.7B parameters makes it quick to download and load
- **Fast Inference**: Good performance on consumer hardware
- **GGUF Format**: Optimized for llama.cpp-based inference engines
- **Q6_K_XL Quantization**: Good balance between model quality and resource usage
- **Proven Compatibility**: Works well with the llama-agent crate

## Memory Requirements

- **RAM**: ~2-3GB for model loading
- **VRAM**: Optional GPU acceleration (can run on CPU)
- **Disk**: ~1.2GB download size

## Usage Notes

This model is specifically recommended for development and testing purposes. It loads quickly, runs efficiently, and provides reasonable responses for testing the HTTP MCP server integration with llama-agent Sessions.

For production use, consider larger models based on your performance and quality requirements.