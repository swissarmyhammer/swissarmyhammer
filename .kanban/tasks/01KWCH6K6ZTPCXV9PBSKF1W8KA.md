---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
project: local-review
title: Extract fleet.rs test module to a sibling file to restore reviewability
---
## Problem

`crates/swissarmyhammer-validators/src/review/fleet.rs` is 135,248 bytes — over the review engine's 131,072-byte batch cap (`a file is never split across review batches`). Result: `review sha HEAD~1..HEAD` **fails outright** whenever this file is in scope, and per-file fallback **skips fleet.rs entirely**. The file holding the core review-fleet logic cannot be reviewed.

Measured breakdown:
- Production code (lines 1–1238): **55,138 bytes** — comfortably under the cap.
- `#[cfg(test)] mod tests` (lines 1239–3180): **80,110 bytes** — 59% of the file.

(Irony: line 95 of this file is `pub const DEFAULT_BATCH_SIZE: usize = 128 * 1024;` — it defines the cap it exceeds.)

## Change

Extract the inline test module into a sibling file. Because `fleet.rs` is a file-module, the idiomatic move:

```rust
// in fleet.rs, replace the inline `mod tests { … }` block with:
#[cfg(test)]
mod tests;
```

Move lines 1239–3180 into `crates/swissarmyhammer-validators/src/review/fleet/tests.rs`. Fix `use super::*;`/path references as needed.

After the split:
- `fleet.rs` → ~55 KB → reviews fine.
- `fleet/tests.rs` → ~80 KB → also under the cap (and is test code, which validators should skip anyway).

**No production logic moves** — blast radius near-zero.

## Scope guard

Test-extraction ONLY. Do NOT refactor production code in this card (the orchestration-vs-rendering split is a separate, optional polish — not this task).

## Done when

- `fleet.rs` is under 131,072 bytes; the test module lives in `review/fleet/tests.rs`.
- `cargo nextest run -p swissarmyhammer-validators` green (proves the move preserved all tests).
- `cargo clippy -p swissarmyhammer-validators --all-targets` zero warnings.
- A `review file` on `fleet.rs` succeeds (no batch-cap error).