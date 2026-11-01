---
assignees:
- claude-code
position_column: todo
position_ordinal: c580
title: Module docs for swissarmyhammer-tools/src/mcp/mod.rs (36 pub mod declarations)
---
36 review findings, all one cause: the `pub mod` block in `crates/swissarmyhammer-tools/src/mcp/mod.rs` (around lines 60-82) has no doc comments.

Split out of ^6xjxebg. Pre-existing — that commit added two `pub use` lines around line 100 and touched the `pub mod` block not at all. This was 67% of that card's 54-finding report.

## Required change

A doc comment on every `pub mod` declaration in the file. One line each, saying what the module provides — not a restatement of the name.

Sweep the whole block. Do not stop at whichever ones a validator happens to cite; the whole point of doing this as one card is that a partial pass makes the next review re-report the remainder.

## Warning on the finding list

Two validators reported the same modules twice, at different line offsets (`43-65` and `68-88` for one list). The real count is ~36 declarations, not 72 distinct defects. Work from the file, not from the finding list.

The engine's cited line numbers track the pre-image and are unreliable on this card — it gave `:160` and `:220` for the same error message elsewhere. Grep, do not trust numbers.

## Acceptance

- Every `pub mod` in `crates/swissarmyhammer-tools/src/mcp/mod.rs` has a doc comment.
- Note the crate does not enable `#![warn(missing_docs)]`, so rustdoc will NOT gate this. Verify with an explicit check (a script, or add the lint) rather than assuming a clean `cargo doc` means done — that mistake was already made once on ^t7ebyn8.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.