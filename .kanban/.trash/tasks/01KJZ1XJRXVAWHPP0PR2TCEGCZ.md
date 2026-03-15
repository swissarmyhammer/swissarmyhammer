---
position_column: done
position_ordinal: a4
title: detection.rs still hardcodes .gguf only for HuggingFace auto-detection
---
**model-loader/src/detection.rs**

`detect_model_type()` only checks for `.gguf` extension, contradicting the runtime-agnostic goal. The `MODEL_EXTENSIONS` constant in `loader.rs` lists `["gguf", "onnx", "mlmodel", "bin", "safetensors"]` but `detection.rs` doesn't use it.

**Fix:**
- [ ] Update `detect_model_type()` to recognize all supported extensions from `MODEL_EXTENSIONS`
- [ ] Consider whether `ModelType` enum needs new variants beyond `Gguf`
- [ ] Update tests to cover new extensions
- [ ] Verify tests pass #review-finding #blocker