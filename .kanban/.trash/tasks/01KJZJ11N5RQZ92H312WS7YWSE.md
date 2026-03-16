---
position_column: done
position_ordinal: m6
title: 'onnxruntime-coreml-sys: build.rs patch_eigen_hash uses hardcoded hashes'
---
**File:** onnxruntime-coreml-sys/build.rs:63-74\n\n**What:** `patch_eigen_hash()` patches a specific SHA1 hash in `cmake/deps.txt` with a hardcoded replacement. This is fragile -- the next ONNX Runtime update will likely change the hash again, silently breaking the build with a confusing cmake fetch error.\n\n**Why:** Hardcoded patch values in build scripts become invisible technical debt. When the ONNX Runtime submodule is updated, this patch may silently become a no-op (if the old hash is gone) or actively harmful (if it corrupts a valid hash).\n\n**Suggestion:** Add a comment documenting which ONNX Runtime version this patch applies to, and consider adding a `cargo:warning` if neither hash is found (indicating the patch is stale and should be reviewed). #review-finding #warning