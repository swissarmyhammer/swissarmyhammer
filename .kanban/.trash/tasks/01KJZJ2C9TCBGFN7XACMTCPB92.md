---
position_column: done
position_ordinal: m9
title: 'onnxruntime-coreml-sys: build.rs collect_build_output silently ignores copy failures'
---
**File:** onnxruntime-coreml-sys/build.rs:198-204, 215-218\n\n**What:** `collect_build_output()` uses `std::fs::copy(&entry, &dest).ok()` to silently discard copy errors for static libraries and headers. If a copy fails (e.g., permission denied, disk full), the build continues but linking will fail later with a confusing error about missing libraries.\n\n**Why:** Silent error suppression in build scripts makes debugging extremely difficult. A failed copy should at minimum emit a `cargo:warning` so users can diagnose build failures.\n\n**Suggestion:** Replace `.ok()` with `.unwrap_or_else(|e| { println!(\"cargo:warning=Failed to copy {}: {}\", entry.display(), e); })` or propagate the error to fail the build immediately with a clear message. #review-finding #warning