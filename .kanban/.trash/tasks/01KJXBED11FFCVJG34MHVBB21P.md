---
position_column: done
position_ordinal: a8
title: 'onnxruntime-coreml-sys: Missing Sync impl for Session and Tensor'
---
File: onnxruntime-coreml-sys/src/lib.rs:513-516

`unsafe impl Send for Session` and `unsafe impl Send for Tensor` are present, but `Sync` is missing for both. The SAFETY comment says "ORT values are thread-safe per documentation" but only implements Send. If Session is truly thread-safe for reads (which it should be since ORT sessions are designed for concurrent inference), it should also be Sync. If it is NOT safe for concurrent reads, the SAFETY comment is misleading.

Also, `Tensor` lacks Sync -- if the intent is to share tensors across threads (e.g. reusing input buffers), this needs Sync.

Suggestion: Add `unsafe impl Sync for Session {}` and `unsafe impl Sync for Tensor {}` if ORT's C API guarantees thread safety for session.Run and tensor reads, or document clearly why Sync is omitted. #review-finding #warning