---
position_column: todo
position_ordinal: d1
title: Rewrite tests and verify all pass
---
Rewrite coreml_test.rs and verify all ane-embedding tests pass with non-zero embeddings.\n\n- [ ] Rewrite coreml_test.rs to use objc2-core-ml directly (no coreml-rs, no ndarray)\n- [ ] integration_test.rs needs no changes (uses public API)\n- [ ] cargo test -p ane-embedding (unit tests)\n- [ ] cargo test -p ane-embedding --test coreml_test (both pass, non-zero embeddings)\n- [ ] cargo test -p ane-embedding --test integration_test (all 8 pass)\n- [ ] cargo clippy -p ane-embedding -- -D warnings (clean)\n- [ ] Verify embedding sum is non-zero (baseline: ~1895 from Python coremltools)