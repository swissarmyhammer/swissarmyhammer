---
position_column: done
position_ordinal: h2
title: get_all_parts in huggingface.rs only handles .gguf multi-part files
---
**model-loader/src/huggingface.rs**

`get_all_parts()` uses a regex `^(.+)-00001-of-(\d{5})\.gguf$` that only matches `.gguf` multi-part files. Other model formats (ONNX, safetensors) have different sharding conventions.

**Fix:**
- [ ] Generalize the multi-part regex to handle multiple extensions or add format-specific patterns
- [ ] Consider safetensors sharding pattern (`model-00001-of-00003.safetensors`)
- [ ] Update tests for multi-format support
- [ ] Verify tests pass #review-finding #blocker