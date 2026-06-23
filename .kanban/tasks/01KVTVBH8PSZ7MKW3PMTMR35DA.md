---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvvbbs2k83e9v1f0v722mq5m
  text: |-
    Picked up. Research done. Studied edit/mod.rs (execute_edit: Applied→commit_content→success vs Ambiguous/NoMatch success-but-no-mutation), write/mod.rs (execute_write: freshness_rebase Some(payload)→non-mutating return vs the post-guard atomic write→mutation), and inline_diagnostics.rs fold_outcome_into_result.

    Envelope representation decision: mirror the established fold_outcome_into_result convention — ride the envelope on BOTH structured_content (a `mutation` object: tagged_content, mutated_paths, plus existing EditResult fields) AND an appended text block that carries the #hash:-prefixed hashline-tagged post-mutation content (via swissarmyhammer_hashline::tag(content, 1)), so a round-trip anchor is immediately available with no intervening read. Keep the existing "OK"/"OK: Applied N..." first text block. Keep context.record_mutated_path side-channel (drives inline diagnostics) — distinct from mutated_paths in the body.

    Paths carrying the envelope: edit ApplyOutcome::Applied→commit path; write post-guard atomic write. Paths deliberately WITHOUT: edit Ambiguous + NoMatch (no mutation), write freshness_rebase divergence (no mutation), all errors.

    Next: TDD — write failing round-trip + envelope-presence + no-envelope-on-non-mutation tests.
  timestamp: 2026-06-23T22:59:36.147623+00:00
- actor: claude-code
  id: 01kvvbs804b5743ykc8h4s99xs
  text: |-
    Implemented + verified. TDD: wrote 7 failing tests first (4 RED for missing structured envelope, 3 already-passing no-envelope guards), then implemented to GREEN.

    Changes:
    - shared_utils.rs: new `mutation_success_response(message, content, mutated_paths, extra)` — the single envelope serializer shared by edit+write. Rides BOTH surfaces (mirrors inline_diagnostics fold_outcome_into_result): structured_content = {mutation: {tagged_content, mutated_paths, ...extra}} AND an appended text block carrying `#hash:<token>` + the hashline-tagged post-mutation content. tagged_content = swissarmyhammer_hashline::tag(content, 1).
    - edit/mod.rs: execute_edit Applied/commit path returns the envelope (extra = bytes_written, replacements_made, encoding_detected, line_endings_preserved, metadata_preserved — all existing EditResult fields preserved). Ambiguous + NoMatch returns unchanged (plain success, no envelope). record_mutated_path side-channel kept.
    - write/mod.rs: execute_write post-guard atomic-write path returns the envelope (extra = bytes_written). freshness_rebase divergence return unchanged. record_mutated_path side-channel kept.

    Design note: kept the envelope at the response boundary (mutation_success_response) rather than adding tagged_content/mutated_paths to the EditResult struct, because EditResult is also produced by the legacy edit_file_atomic + commit_content (shared, no tagged-content concept). Layering at the response boundary keeps the hashline tag in ONE place (no duplication) and lets write reuse it. Acceptance criteria are on the RESULT carrying the fields — satisfied.

    Envelope carried on: edit Applied→commit; write atomic-write. Deliberately NOT on: edit ambiguity, edit near-miss, write freshness-rebase divergence, all errors (no mutation).

    Round-trip test (both edit→edit and write→edit): pull the N:HH anchor straight from the prior result's tagged_content and use it as `find` in an immediately-following edit files call — resolves against on-disk, no intervening read.

    Strengthened (not weakened): test_edit_response_format + test_write_success_response_format now assert on-disk mutation happened AND the structured envelope describes it, still asserting content[0]=="OK".

    Tests: `cargo nextest run -p swissarmyhammer-tools edit:: write:: files::` → 244 passed. `cargo nextest run -p swissarmyhammer-tools` → 1378 passed, 0 failed. `cargo test -p swissarmyhammer-tools --doc` → 8 passed. `cargo fmt` applied, `cargo clippy -p swissarmyhammer-tools -- -D warnings` clean. double-check agent: PASS.

    Moving to review.
  timestamp: 2026-06-23T23:06:57.412491+00:00
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
- 01KVTVACR1W8HFKFR8DTAJPMP0
- 01KVTVBS4R1P2F351KGKVRPXPZ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdb80
project: file-edit-tools
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