---
position_column: done
position_ordinal: m3
title: 'onnxruntime-coreml-sys: Env, Session, Tensor missing Send+Sync for SessionOptions'
---
**File:** onnxruntime-coreml-sys/src/lib.rs:634-641\n\n**What:** `unsafe impl Send` and `unsafe impl Sync` are declared for `Env`, `Session`, and `Tensor`, and `Send` (but not `Sync`) for `SessionOptions`. The `SessionOptions` exclusion of `Sync` appears intentional, but there is no SAFETY comment explaining why `SessionOptions` is `Send` but not `Sync`.\n\n**Why:** Per Rust review guidelines, Send/Sync impls on pointer-wrapping types need compile-time assertions or clear safety documentation. The existing comment (line 632-633) only covers `Env`, `Session`, and `Tensor`. `SessionOptions` is quietly different.\n\n**Suggestion:** Add an explicit SAFETY comment for `SessionOptions` explaining why it is `Send` but not `Sync`, or add `Sync` if it is actually thread-safe. #review-finding #warning