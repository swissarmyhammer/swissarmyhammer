---
position_column: done
position_ordinal: h8
title: ModelError Display messages violate Rust conventions
---
**model-loader/src/error.rs**

`user_friendly_message()` uses emojis and multi-line strings, which is fine for user-facing output. However, the `Display` impl (via `#[error(...)]`) also uses title case and emojis in some variants. Rust convention is lowercase, no punctuation for `Display` on errors.

**Fix:**
- [ ] Ensure `#[error(...)]` messages are lowercase, single-line, no emoji
- [ ] Keep `user_friendly_message()` as-is for user-facing output
- [ ] Verify tests pass #review-finding #warning