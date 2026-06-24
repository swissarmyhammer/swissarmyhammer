---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvveh33z34dj5h8v6hwdd5g0
  text: |-
    Picked up. Research done on current edit/mod.rs.

    Findings:
    - Legacy no-op check: execute_edit() pre-validation loop rejects `find == replace` with "Edit operation N: old_text and new_text must be different" (hard McpError). Two tests assert substring "must be different"/"different". This IS the no-op rejection concept — will keep as the single coherent path, improve the message, keep tests green.
    - Cascade resolve loop: apply_all_pairs() -> resolve_pair() -> PairOutcome {Resolved(Resolution), Ambiguous, NoMatch}. ApplyOutcome {Applied, Ambiguous, NoMatch}. NoMatch/Ambiguous short-circuit, file byte-identical, surfaced as SUCCESSFUL results.
    - Plan: add PairOutcome::AlreadyApplied (replace present + find absent) and PairOutcome::ConsumedTarget (later pair's target span consumed by earlier pair in same batch). Both surface as informational SUCCESS results via new ApplyOutcome variants. Detection runs in resolve phase before commit, preserving atomicity + existing ambiguity/near-miss behavior.

    Following /tdd.
  timestamp: 2026-06-23T23:54:55.999438+00:00
- actor: claude-code
  id: 01kvvexntd5tct872way9rwxe4
  text: |-
    Implementation landed (TDD: 3 tests written RED first — no_op passed immediately since legacy check already covers it; already_applied + consumed_target failed as expected, then implemented to GREEN).

    Changes in crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs:
    - No-op rejection: reconciled the legacy `find == replace` check in execute_edit's validation loop — single coherent path, clearer message ("no-op edit — `find` and `replace` are identical..."). No second code path. Existing "must be different" tests stay green (message still contains "different").
    - Already-applied: new ApplyOutcome::AlreadyApplied + render_already_applied_prompt. When a pair's find is absent and replace (non-empty) is present in working content, reports "likely already applied" as informational SUCCESS, file byte-identical, no mutation envelope.
    - Consumed-target: new ApplyOutcome::ConsumedTarget + render_consumed_target_prompt. New reclassify_no_match() helper runs in apply_all_pairs (which holds both original + working). When a later pair's find is absent from working but present in original AND working != original, reports the consumed-target case per-edit with the original line number.
    - Detection runs in the resolve-all phase before commit (reclassify_no_match intercepts PairOutcome::NoMatch in apply_all_pairs); both new outcomes short-circuit before commit_content, so atomicity holds and ambiguity/near-miss paths are untouched.
    - Updated edit/description.md with a "Safety and idempotency" section.

    Precedence in reclassify_no_match (most-benign first): already-applied (guarded by !replace.is_empty() so delete isn't mis-detected) → consumed-target (guarded by working != original so no false positive) → near-miss.

    Verification (fresh):
    - cargo nextest run -p swissarmyhammer-tools edit:: → 70 passed, 0 failed
    - cargo nextest run -p swissarmyhammer-tools files:: → 227 passed
    - cargo nextest run -p swissarmyhammer-tools → 1361 passed, 0 failed
    - cargo test -p swissarmyhammer-tools --doc → 8 passed
    - cargo clippy -p swissarmyhammer-tools -- -D warnings → clean
    - cargo fmt applied
    - double-check agent: PASS (verified atomicity, no-op single path, precedence soundness, no regressions).

    No prior edit tests needed changing — all stayed green. Moving to review.
  timestamp: 2026-06-24T00:01:48.365388+00:00
depends_on:
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdd80
project: file-edit-tools
title: edit files — idempotency, no-op rejection, and consumed-target detection
---
## What
Follow-on to the cascade core. Add the safety/idempotency semantics to `edit files` in `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`, layered on the resolved-pair apply loop.

- **No-op rejection**: reject a pair where `find == replace`.
- **Already-applied detection**: if a pair's `replace` text is already present in the file and `find` is absent, report "likely already applied" (informational, not a hard error) rather than failing with "not found".
- **Consumed-target detection**: in a multi-pair batch, detect when a later pair's resolved target span was consumed/overwritten by an earlier pair in the same batch, and report that specific case (per-edit) instead of a generic miss. (Doc line 211.)

These operate on the atomic batch from the cascade-core task, so they must run during the resolve-all phase before commit; no double-apply.

## Acceptance Criteria
- [ ] No-op (`find == replace`) is rejected with a clear message.
- [ ] `replace` present + `find` absent reports "already applied" (not a hard "not found" error).
- [ ] A later pair whose target was consumed by an earlier pair in the same batch is detected and reported per-edit; the batch remains atomic (file byte-identical on failure).

## Tests
- [ ] Unit tests: no-op rejection; already-applied path; consumed-target detection with byte-identical file on the failing batch.
- [ ] `cargo test -p swissarmyhammer-tools edit::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.