---
position_column: done
position_ordinal: h4
title: ModelResolver stores dead retry_config field
---
**model-loader/src/loader.rs**

`ModelResolver` stores `retry_config` in its struct, but `resolve()` reads `config.retry_config` from the `ModelConfig` parameter instead. The struct field is never used — it's dead state.

**Fix:**
- [ ] Remove `retry_config` field from `ModelResolver` struct
- [ ] Simplify `ModelResolver::new()` — no longer needs to accept or store retry config
- [ ] Verify tests pass #review-finding #warning