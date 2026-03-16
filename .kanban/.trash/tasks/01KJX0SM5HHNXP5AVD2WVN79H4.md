---
position_column: done
position_ordinal: a3
title: Rename llama-loader to model-loader (or extract shared ModelSource)
---
Address the fact that llama-loader is really a general-purpose model loader. Two approaches — pick during implementation:

**Option A: Rename to model-loader**
- Rename the crate from `llama-loader` to `model-loader`
- Keep the same functionality (HuggingFace + Local source, download coordination, retry, caching)
- The `LoadedModel` return type is the challenge — it currently returns `LlamaModel` which is llama-cpp-specific
- Could make `LoadedModel` return a file path + metadata, letting the consumer (llama-embedding, ane-embedding) load into their runtime

**Option B: Extract ModelSource + download into model-loader, keep llama-loader as thin wrapper**
- Create `model-loader` with just `ModelSource`, download coordination, `ModelMetadata`, `RetryConfig`
- `model-loader::load()` returns `ResolvedModel { path: PathBuf, metadata: ModelMetadata }` — just the file, not loaded into any runtime
- `llama-loader` becomes a thin wrapper: takes `ResolvedModel` → loads into `LlamaModel` via llama-cpp-2
- `ane-embedding` uses `model-loader` directly to get the ONNX file path, then loads via `ort`

**Recommendation: Option B** — it's less disruptive and the right separation of concerns. The download/caching/source-resolution logic is truly generic. The "load into runtime" step is backend-specific.

**Checklist:**
- [ ] Create `model-loader/` crate with `ModelSource`, download coordination, `ResolvedModel`, `ModelMetadata`
- [ ] Move HuggingFace download logic, local file resolution, retry, caching from llama-loader
- [ ] `ResolvedModel` returns `{ path: PathBuf, metadata: ModelMetadata }` — no runtime coupling
- [ ] Update llama-loader to depend on model-loader, become a thin llama-cpp-2 loading wrapper
- [ ] Update llama-embedding to use model-loader for source resolution (if needed, or keep using llama-loader)
- [ ] All existing llama-embedding tests still pass
- [ ] Run tests