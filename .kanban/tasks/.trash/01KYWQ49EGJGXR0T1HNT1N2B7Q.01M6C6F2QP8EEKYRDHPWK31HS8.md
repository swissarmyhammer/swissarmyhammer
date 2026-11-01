---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: 'swissarmyhammer-skills lib.rs: document the module declarations and re-exports'
---
18 missing-doc findings on `crates/swissarmyhammer-skills/src/lib.rs`, lines 19-43 — the `mod` declarations and `pub use` re-exports. Split out of ^qsr5rdt's review; all pre-existing.

The single line that commit changed in this file was dropping `SAH_INTERNAL_FRONTMATTER_KEYS` from an already-undocumented `pub use`. The missing docs predate it.

## Required change

A doc comment on every `mod` declaration and every `pub use` in the file. One line each, saying what it provides — not a restatement of the name.

Sweep the whole file. A partial pass makes the next review re-report the remainder, which is exactly how this file accumulated 18 findings.

## Note on verification

The crate does not enable `#![warn(missing_docs)]`, so rustdoc will NOT gate this. Verify with an explicit check — a script, or add the lint. That mistake was already made once on ^t7ebyn8, where `cargo doc --no-deps` was cited as proof and had never checked for missing docs at all.

Consider whether adding `#![warn(missing_docs)]` to the crate is the real fix, so this cannot silently regrow. If you add it, expect other files in the crate to light up; either fix them in this card or say what is left.

## Acceptance

- Every `mod` and `pub use` in `lib.rs` has a doc comment.
- Verified by an explicit check, not by a clean `cargo doc`.
- `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Sibling card: ^mz0s6bf is the same defect class in `swissarmyhammer-tools/src/mcp/mod.rs` (36 findings). Same approach applies; do them together if convenient. #refactor