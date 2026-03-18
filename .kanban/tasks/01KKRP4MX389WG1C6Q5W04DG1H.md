---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff9180
title: 'heb: HebError error messages have uppercase first letter, violating Rust convention'
---
heb/src/error.rs:6-17

```rust
#[error("Database error: {0}")]
#[error("Serialization error: {0}")]
#[error("Election error: {0}")]
#[error("IO error: {0}")]
```

Per the Rust `thiserror`/`Display` convention (and the project's own RUST_REVIEW.md guideline), `Display` messages on errors should be lowercase with no trailing punctuation. These messages start with uppercase and include a colon separator — e.g., `"Database error: ..."` should be `"database error: ..."`.

Similarly in `ElectionError` in swissarmyhammer-leader-election/src/error.rs: `"Discovery file error: {0}"`, `"Bus error: {0}"`, `"Serialization error: {0}"`, `"Message error: {0}"` are all uppercase.

Suggestion: lowercase the first character of each error message string. #review-finding