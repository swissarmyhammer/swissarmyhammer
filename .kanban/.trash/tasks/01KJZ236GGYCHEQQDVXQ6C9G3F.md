---
position_column: done
position_ordinal: h9
title: Add runtime compatibility check in llama consumers after model resolution
---
**llama-agent/src/model.rs, llama-embedding/src/model.rs**

After `ModelResolver::resolve()` returns a generic path, llama consumers must verify the resolved file is a `.gguf` before attempting `LlamaModel::load_from_file()`. Currently they blindly pass the path through.

**Fix:**
- [ ] Add a check after `resolve()` that the resolved path has a `.gguf` extension
- [ ] Return a clear error if the model format is incompatible (e.g., "llama-cpp-2 only supports .gguf models, got .onnx")
- [ ] Apply to both llama-agent and llama-embedding
- [ ] Verify tests pass #review-finding #blocker