---
assignees:
- claude-code
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
- 01KVTVACR1W8HFKFR8DTAJPMP0
- 01KVTVBS4R1P2F351KGKVRPXPZ
position_column: todo
position_ordinal: a880
project: null
title: Shared mutating-result contract — tagged_content + mutated_paths in the result body
---
## What
Extend the result of mutating file ops so the model can chain edits without re-reading. In `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` (`EditResult`) and the `write files` result (`write/mod.rs`):
- Add `tagged_content: String` — a re-tagged (hashline) view of the changed file after the edit, produced via `swissarmyhammer_hashline::tag`, so fresh anchors are immediately available for the next edit.
- Add `mutated_paths: Vec<String>` to the **result body** (distinct from the existing typed `record_mutated_path` side-channel used for diagnostics — this surfaces them to the model).
- Keep all existing fields: `bytes_written`, `replacements_made`, `encoding_detected`, `line_endings_preserved`, `metadata_preserved`.
- Serialize these into the `CallToolResult` content (structured) so they reach the model.
- Note: inline diagnostics are still folded in by the existing `inline_diagnostics.rs` chokepoint via the side-channel — do not duplicate that here.

**Ordering note (why this depends on the write-guard task):** this task and the write-guard task both edit `write/mod.rs`. It now depends on the write-guard task so the two `write/mod.rs` reworks serialize; apply the result envelope on top of the guard's divergence/return-current-content path (a divergence return is not a successful mutation, so it carries no `tagged_content`/`mutated_paths`).

## Acceptance Criteria
- [ ] A successful `edit files` result includes `tagged_content` (hashline-tagged post-edit file) and `mutated_paths`.
- [ ] A successful `write files` result includes the same `tagged_content` + `mutated_paths`; a guard-divergence (non-mutating) write does NOT.
- [ ] Anchors taken from `tagged_content` resolve against the on-disk file in an immediately-following `edit files` call (round-trip test).
- [ ] Existing result fields remain present and correct.

## Tests
- [ ] Unit tests: edit/write result carries `tagged_content` + `mutated_paths`; a chained edit using an anchor from the prior result's `tagged_content` succeeds without an intervening read; guard-divergence write omits the envelope.
- [ ] `cargo test -p swissarmyhammer-tools` (files module) is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.