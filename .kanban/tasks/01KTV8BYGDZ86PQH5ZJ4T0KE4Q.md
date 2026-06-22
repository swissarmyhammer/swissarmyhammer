---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvqqy9t7ne0jynh2qqt9cv9x
  text: |-
    Picked up. On inspection, most of this fix was already in place in dispatch.rs: `resolve_depends_on` delegates to a `depends_on_refs(value)` helper that already handles (a) stringified-array string -> parsed array, (b) plain string -> single ref, (c) non-string/non-array value -> KanbanError::parse, with every ref routed through resolve_task_ref. Existing tests cover those: dispatch_update_task_depends_on_stringified_array_persists, _single_string_persists, _caret_single_string_persists_full_ulid, _unresolvable_ref_errors, _malformed_scalar_errors_without_clearing.

    The ONE spec requirement still unmet: a non-string entry INSIDE an array was silently skipped via `filter_map(|v| v.as_str()...)`. Spec requires a clear KanbanError::parse naming the offending entry, no silent skip.

    TDD: added failing test dispatch_update_task_depends_on_non_string_array_entry_errors_without_clearing (seeds a real dep, then sends depends_on: [dep_id, 42], asserts err + seeded dep survives read-back). Watched it FAIL (the 42 was silently dropped so the update succeeded). Then changed the array branch of depends_on_refs from filter_map to `.map(|v| v.as_str()...ok_or_else(|| KanbanError::parse("depends_on array entries must be task ref strings, got: {v}")))` collecting into Result. Watched it pass (GREEN).
  timestamp: 2026-06-22T13:22:28.295594+00:00
- actor: claude-code
  id: 01kvqqzt0r3xk2xb24266xg74a
  text: |-
    Acceptance gates (all run fresh, this session):

    - `cargo nextest run -p swissarmyhammer-kanban` -> "1402 tests run: 1402 passed (1 slow, 1 leaky), 0 skipped". Includes all 13 depends_on tests (the new dispatch_update_task_depends_on_non_string_array_entry_errors_without_clearing among them).
    - `cargo clippy -p swissarmyhammer-kanban -- -D warnings` -> exit 0, "Finished" with no warnings.
    - `cargo fmt -p swissarmyhammer-kanban` -> clean.

    Note: `cargo clippy -p swissarmyhammer-kanban --all-targets` surfaces a PRE-EXISTING too_many_arguments error in the unrelated swissarmyhammer-focus crate (focus_lost). Not touched by this task; the task's specified gate (without --all-targets) is clean.

    Edge-case review of depends_on_refs: empty array -> Ok(empty) (explicit clear-all, legitimate); [null] / nested array / [valid, 42] -> error naming offending entry; stringified-array string -> parsed; plain string -> single ref; non-string non-array scalar -> error. All five spec coercion bullets satisfied; every ref still routes through resolve_task_ref so garbage becomes TaskNotFound and never reaches with_depends_on.

    Moving to review.
  timestamp: 2026-06-22T13:23:17.656855+00:00
- actor: claude-code
  id: 01kvqr15dkr4yhedt2hznxjsam
  text: |-
    Adversarial double-check VERDICT: PASS. It independently verified every spec clause, the requested edge cases (empty array, nested array, null entry, mixed valid+invalid), and the "without clearing" guarantee (resolve_depends_on runs with `?` before the command is built, so an error returns before any mutation — pre-existing deps preserved). Its own fresh runs: `cargo test -p swissarmyhammer-kanban --lib depends_on` 11 passed 0 failed; `cargo clippy -p swissarmyhammer-kanban --lib` no warnings/errors.

    Two non-blocking observations from the critic (no action needed):
    1. add_task guards `!dep_ids.is_empty()` (empty array = no-op on add) while update_task does not (empty array clears deps on update). Both reasonable, neither is the silent-drop bug.
    2. A stringified array of non-strings (e.g. "[\"a\", 42]") fails the Vec<String> parse and falls to the single-ref branch, yielding a TaskNotFound error rather than the "must be strings" message. Still an error (no silent drop); stringified-non-string-array is not a spec requirement.
  timestamp: 2026-06-22T13:24:02.099381+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd580
title: 'Fix silent drop: depends_on passed as string / stringified array is ignored by resolve_depends_on'
---
## What
`resolve_depends_on` in `crates/swissarmyhammer-kanban/src/dispatch.rs` (the helper used by BOTH the `add task` and `update task` dispatch arms) silently drops `depends_on` input that is not a JSON array of strings:

- A plain string (`"depends_on": "^abc1234"`) or a JSON-stringified array (`"depends_on": "[\"01KT…\"]"`) fails the `op.get_param("depends_on").and_then(|v| v.as_array())` guard → `Ok(None)` → the param is silently ignored. The caller believes the dependency was set; nothing happened.
- Non-string entries inside an array are skipped silently by the `if let Some(s) = v.as_str()` loop body.

Observed on a real board: a garbage literal `'["01KT…"]'` (a JSON-stringified array as a single string) stored verbatim as a dependency id. Forgiving input must coerce or return a clear error — never silently drop, and garbage must never be storable as a dep id.

Fix — coerce, consistent with the op system's documented forgiving input ("aliases and inference") and the existing single-string `assignee` → `assignees` fallback in `resolve_assignees` (same file):
- [ ] String value that parses as a JSON array via `serde_json::from_str` → treat as that array (the stringified-array case).
- [ ] Any other string → single-element list (one forgiving task ref).
- [ ] Array entry that is not a string → clear `KanbanError::parse` naming the param and the offending entry (no silent skip).
- [ ] Any other JSON type (number, bool, object) → clear `KanbanError::parse` naming the param and expected shape.
- [ ] Every ref continues through `resolve_task_ref`, so an unresolvable/garbage ref is a `TaskNotFound` error — a garbage literal can never reach `with_depends_on` and be stored.

## Acceptance Criteria
- [ ] `update task` with `depends_on` as a JSON-stringified array string resolves and stores the canonical ULID list (read back via `get task`).
- [ ] `update task` / `add task` with `depends_on` as a plain string (short id, `^short`, or full ULID) stores a single-element canonical list.
- [ ] `depends_on` with a non-string array entry, or a non-string/non-array value that is neither a parseable array nor a resolvable ref, returns a clear `KanbanError` — and a `get task` read-back shows `depends_on` unchanged (nothing stored).
- [ ] Existing array-of-strings behavior unchanged (regression: `dispatch_add_task_with_depends_on`, `dispatch_update_task_with_depends_on`, short-id persistence tests still green).
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] Dispatch tests in `crates/swissarmyhammer-kanban/src/dispatch.rs` `#[cfg(test)]` (existing TempDir board pattern, cf. `dispatch_update_task_with_depends_on`): stringified-array string input → deps stored as canonical ULIDs (the garbage-literal regression — fails before the fix because the param is silently dropped); plain-string input → single-element list; non-string array entry → error; garbage string → `TaskNotFound` error and `depends_on` unchanged on read-back.
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the failing stringified-array and plain-string coercion tests first, then implement the coercion in `resolve_depends_on`. #bug