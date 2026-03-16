---
position_column: done
position_ordinal: a0
title: Solve ONNX Runtime + CoreML build story (onnxruntime-coreml-sys or ort config)
---
The biggest risk in the ANE embedding plan: getting `ort` to use CoreML execution provider with just `cargo build`.

**The problem (from ideas/ane-embed-crate-plan.md):**
- `ort`'s `download-binaries` default doesn't include CoreML
- CoreML requires either hand-building ONNX Runtime or the `compile` strategy (20+ min)
- Neither is acceptable for dev experience

**Options to investigate:**
1. **ort's `load-dynamic` feature** — skip link-time, `dlopen` at runtime. Build ONNX Runtime with CoreML once, point to dylib. Simplest starting point.
2. **onnxruntime-coreml-sys crate** — vendor ONNX Runtime as submodule, build.rs compiles with `--use_coreml`. Follow llama-cpp-sys-2 pattern.
3. **Prebuilt binaries** — host CoreML-enabled ONNX Runtime dylibs on GitHub Releases, download in build.rs.
4. **Check if ort v2.x has improved** — pyke may have added CoreML prebuilts since the plan was written.

**Recommendation from plan:** Start with option 1 (`load-dynamic`), move to option 2 if needed.

**Key risk:** ONNX Runtime cmake may require Python for protobuf codegen. Test this early.

**Checklist:**
- [ ] Test `ort` v2.x with `coreml` feature — does it work out of the box now?
- [ ] If not, prototype `load-dynamic` approach with manually-built ONNX Runtime
- [ ] If build-from-source needed, create `onnxruntime-coreml-sys/` crate skeleton
- [ ] Verify CoreML EP activates and routes to ANE (not just CPU)
- [ ] Document the build story for contributors
- [ ] Run smoke test: load a small ONNX model, get embedding via CoreML EP