---
position_column: done
position_ordinal: i3
title: Fix initial_delay_ms u128 to u64 truncation
---
**model-loader/src/retry.rs**

`initial_delay_ms` is computed as `u128` (from `Duration::as_millis()`) but cast to `u64`. For practical values this is fine, but it's technically lossy.

- [ ] Use `as_millis() as u64` with a comment, or use `try_into().unwrap_or(u64::MAX)`
- [ ] Verify tests pass #review-finding