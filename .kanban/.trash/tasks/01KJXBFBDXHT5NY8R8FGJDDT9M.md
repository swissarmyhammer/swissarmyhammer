---
position_column: done
position_ordinal: b3
title: 'onnxruntime-coreml-sys: OrtError missing standard trait impls'
---
File: onnxruntime-coreml-sys/src/lib.rs:180-193

OrtError implements Debug, Display, and Error but is missing Clone, PartialEq, and Eq. Per Rust review guidelines, new public types should implement all applicable traits. OrtError contains only a u32 and String -- both are Clone + PartialEq, so these should be derived.

Similarly, Env, SessionOptions, Session, and Tensor are missing Debug impls. At minimum, Debug should be implemented for all public types.

Suggestion: Add `#[derive(Clone, PartialEq, Eq)]` to OrtError. Add Debug impls (even if just printing "Env(...)" for opaque pointer types) to Env, SessionOptions, Session, Tensor. #review-finding #warning