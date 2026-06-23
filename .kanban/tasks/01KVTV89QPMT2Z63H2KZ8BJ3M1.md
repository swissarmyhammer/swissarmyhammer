---
assignees:
- claude-code
position_column: todo
position_ordinal: a180
project: null
title: swissarmyhammer-hashline crate — pure hashline core (hash/tag/parse/apply)
---
## What
Create a new pure, IO-free crate `crates/swissarmyhammer-hashline` and add it to the workspace `members` in the root `Cargo.toml`. It provides the hashline anchor primitives used by `read files` (to tag lines) and `edit files` (to resolve anchors). No file IO in this crate.

Public API (in `src/lib.rs` + small submodules):
- `hash_line(&str) -> u8` rendered as 2 lowercase hex chars by a helper: `crc32fast` over the line with **leading/trailing horizontal whitespace (spaces and tabs) stripped, interior preserved**, then `mod 256`. 256 values = staleness detection, not uniqueness; the line number disambiguates collisions.
- `tag(content: &str, start_line: usize) -> String` — annotate each line as `N:HH|line` where N is the absolute 1-based line number (= `start_line` for the first line).
- `parse_anchor(s: &str) -> Option<(usize, u8)>` — parse `N:HH`; tolerate an optional `|text` suffix (ignored here, used by the caller for verification/fallback).
- `apply(content: &str, ops: &[AnchorOp]) -> Result<Applied, HashlineError>` where `AnchorOp { line: usize, hash: u8, replacement: String }`: resolve each op to a line (exact line N first; else proximity-search nearby lines for one hashing to HH), reject on mismatch returning current re-tagged content in the error, preserve original line endings and encoding. Reuse/port the line-ending detection from `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` (`LineEnding::detect`) — lift a copy here rather than depending on the tools crate (keep this crate dependency-light: `crc32fast` only).

## Acceptance Criteria
- [ ] `cargo build -p swissarmyhammer-hashline` succeeds; crate is a workspace member with no dependency on `swissarmyhammer-tools`.
- [ ] `hash_line` strips only horizontal whitespace (a re-indented line hashes identically; an interior change hashes differently).
- [ ] `tag("a\nb", 1)` yields `1:HH|a\n2:HH|b` with correct per-line hashes.
- [ ] `parse_anchor("42:a3")` and `parse_anchor("42:a3|text")` both yield `(42, 0xa3)`; non-anchors yield `None`.
- [ ] `apply` resolves an exact-line anchor, finds a drifted anchor by proximity, and rejects a true hash mismatch with re-tagged current content in the error.

## Tests
- [ ] Property tests in `crates/swissarmyhammer-hashline/tests/` (proptest or hand-rolled perturbation): tag→edit→re-tag round-trips; mismatch rejects; proximity finds a drifted anchor; reformatting (re-indentation) preserves anchors.
- [ ] Unit tests for `hash_line`, `parse_anchor`, `tag` edge cases (empty content, trailing newline, CRLF).
- [ ] `cargo test -p swissarmyhammer-hashline` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.