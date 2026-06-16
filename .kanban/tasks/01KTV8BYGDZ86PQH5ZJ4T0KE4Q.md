---
assignees:
- claude-code
position_column: todo
position_ordinal: 9a80
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