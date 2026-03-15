---
position_column: done
position_ordinal: h5
title: auto_detect_model_file returns nondeterministic results
---
**model-loader/src/loader.rs**

`auto_detect_model_file()` iterates directory entries without sorting. If multiple model files exist, the result depends on filesystem enumeration order, which varies across platforms and runs.

**Fix:**
- [ ] Sort candidates before selecting (e.g., by name, or by extension priority)
- [ ] Add a test verifying deterministic selection when multiple files match
- [ ] Verify tests pass #review-finding #warning