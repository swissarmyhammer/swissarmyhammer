---
position_column: done
position_ordinal: m4
title: 'onnxruntime-coreml-sys: Tensor shape() hardcoded max 8 dims with no overflow check'
---
**File:** onnxruntime-coreml-sys/src/lib.rs:610-621\n\n**What:** `Tensor::shape()` uses a stack-allocated array of 8 i64 values and passes it to the C wrapper. If the ONNX model has a tensor with more than 8 dimensions, the C code will write out of bounds.\n\n**Why:** While 8 dims covers nearly all practical ONNX models, the C wrapper `ort_wrapper_get_tensor_shape` could write beyond the buffer if `num_dims > 8`. This is undefined behavior.\n\n**Suggestion:** Either (1) check `num_dims <= 8` after the C call and return an error if exceeded, or (2) query the number of dimensions first and dynamically allocate. Option 1 is simplest and sufficient. #review-finding #warning