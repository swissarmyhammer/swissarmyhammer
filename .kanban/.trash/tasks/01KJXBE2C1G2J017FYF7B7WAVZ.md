---
position_column: done
position_ordinal: a6
title: 'onnxruntime-coreml-sys: Tensor::from_f32 uses CreateTensorWithDataAsOrtValue but data lifetime is not tied to tensor'
---
File: onnxruntime-coreml-sys/src/lib.rs:436 and wrapper.c:258

CreateTensorWithDataAsOrtValue does NOT copy the data -- the OrtValue just wraps the caller's buffer. The Rust `Tensor::from_f32` takes `data: &[f32]` but the resulting Tensor can outlive the borrowed slice. After the borrow expires, the ORT tensor holds a dangling pointer.

The doc comment on line 435 says "The data must outlive the tensor (the tensor references it, doesn't copy)" but the signature takes a temporary borrow with no lifetime tie. The caller can drop the data Vec while the Tensor is still alive.

Suggestion: Either (a) make Tensor own the data (store a Vec alongside the raw pointer) and pass &self.data to ORT, or (b) add a lifetime parameter `Tensor<'a>` so the compiler enforces the borrow. #review-finding #blocker