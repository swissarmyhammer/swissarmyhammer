---
comments:
- actor: wballard
  id: 01kvat51y4s9rdc5hp7mnbbwt1
  text: 'Picked up. Research done: confirmed root cause at resolve_depends_on in crates/swissarmyhammer-kanban/src/dispatch.rs — gates on `.as_array()` and silently drops single-string / stringified-array shapes. resolve_assignees (same file) is the forgiving pattern to mirror; resolve_task_ref already normalizes all id formats. Real-path e2e harness: inline #[cfg(test)] mod tests in crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs uses create_test_context().with_working_dir() + KanbanTool::execute(). Writing failing tests first (TDD RED).'
  timestamp: 2026-06-17T12:50:59.140191+00:00
- actor: wballard
  id: 01kvav73w4v5gwxq8yp3yhk391
  text: |-
    Fix landed (GREEN). resolve_depends_on in crates/swissarmyhammer-kanban/src/dispatch.rs now extracts refs via a new depends_on_refs() helper accepting: JSON array, single string, and stringified JSON array (serde_json::from_str::<Vec<String>> with single-ref fallback). Every ref routes through resolve_task_ref so all id formats normalize to canonical ULID; unresolvable refs error (no silent drop). Both add task and update task share the helper.

    RED->GREEN evidence:
    - 4 dispatch unit tests (single-string, stringified-array, ^-prefixed single, unresolvable-errors) FAILED before the fix (depends_on read back as [], or error not raised), PASS after.
    - 3 real-path e2e tests in crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs driving KanbanTool::execute() over a temp board FAILED before, PASS after. The shapes-matrix test covers id formats {full ULID, short id, ^<short>, 12-char prefix, lowercase} x shapes {single string, JSON array, stringified array} and asserts round-trip as canonical ULID.

    Docs updated: AddTask/UpdateTask depends_on field docs (schema param descriptions) + crates/swissarmyhammer-tools/src/mcp/tools/kanban/description.md now state depends_on accepts single-ref-or-list in any id format/shape and blocked_by is derived (unsatisfied subset of depends_on), not directly settable.

    Results: cargo nextest -p swissarmyhammer-kanban 1503 passed/0 failed; -p swissarmyhammer-tools 1255 passed/0 failed; clippy -p both --all-targets -D warnings clean; cargo build (workspace) clean.
  timestamp: 2026-06-17T13:09:35.236102+00:00
- actor: wballard
  id: 01kvavqhprv1egy046gdy4mahp
  text: 'Adversarial double-check (really-done gate) returned REVISE with one low-severity finding: a non-string, non-array scalar (e.g. depends_on: 42) fell through to Vec::new() => Ok(Some(vec![])), which on update would CLEAR existing deps — a new variant of the silent-drop anti-pattern. Fixed: depends_on_refs now returns Result and errors (KanbanError::parse) on a value that is neither a string nor an array, consistent with the task''s error-not-drop intent. Added TDD test dispatch_update_task_depends_on_malformed_scalar_errors_without_clearing (RED before, GREEN after) asserting the malformed scalar errors AND the pre-existing dependency survives. Re-verified: kanban 1504 passed/0 failed, clippy -p swissarmyhammer-kanban --all-targets -D warnings clean.'
  timestamp: 2026-06-17T13:18:33.688256+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffba80
title: 'Fix silent drop: depends_on persists only for literal JSON arrays — accept all id formats + string/array shapes, add real-path test'
---
## Context

Multiple agents report — and I reproduced live against the running `kanban` MCP tool — that setting `depends_on` (and `blocked_by`) silently no-ops: the call returns `{ok:true}` but `get task` reads the field back as `[]`. Reproduced: `update task` with `depends_on: ["<ulid>"]` → ack ok → `get task` shows `depends_on: []`.

**Root cause (code):** `resolve_depends_on` in `crates/swissarmyhammer-kanban/src/dispatch.rs` gated on `op.get_param("depends_on").and_then(|v| v.as_array())` and returned `Ok(None)` for anything that is not a literal JSON array. So a single id string, a `^`-prefixed string, or a stringified array were all silently dropped.

`blocked_by` is a DERIVED field (the unsatisfied subset of `depends_on`), not directly settable. The canonical input is `depends_on`; this task makes that robust and documents it.

## What

- [x] In `crates/swissarmyhammer-kanban/src/dispatch.rs`, make `resolve_depends_on` forgiving about input shape, mirroring `resolve_assignees`:
  - [x] JSON array of refs → resolve each via `resolve_task_ref`.
  - [x] single JSON string → treat as a one-element list.
  - [x] stringified JSON array (`"[\"…\"]"`) → parse with `serde_json::from_str` to a `Vec<String>`, then resolve each; if it doesn't parse as an array, fall back to treating the whole string as one ref.
  - [x] Every element routes through `resolve_task_ref`; an unresolvable ref errors (NOT silently dropped).
  - [x] Both `add task` and `update task` call this one helper — verified both paths.
- [x] Update the kanban `description.md` and the `depends_on` operation param docs to state `depends_on` accepts a single ref or a list in any id format, and that `blocked_by` is derived and not directly settable.

## Acceptance Criteria
- [x] `depends_on` persists when supplied as: a JSON array of ids, a single id string, and a stringified JSON array — verified by reading the task back.
- [x] Each of these id formats resolves and persists as the canonical full ULID: full ULID, 7-char short id, `^<short>`, unique ULID prefix, lowercase ULID.
- [x] An unresolvable `depends_on` ref returns an error (not a silent empty).
- [x] A malformed (non-string, non-array) `depends_on` errors rather than silently clearing existing deps (double-check finding).
- [x] `add task` and `update task` both honor `depends_on`; behavior with no `depends_on` is unchanged.
- [x] Docs state `depends_on` is the input (any format/shape) and `blocked_by` is derived.

## Tests
- [x] Real-path e2e in `swissarmyhammer-tools` driving `KanbanTool::execute()` over a temp board: shapes-matrix (5 id formats × 3 wire shapes) + add-task single-string + unresolvable-errors. RED before fix, GREEN after.
- [x] Dispatch-level unit tests in `dispatch.rs`: single-string, stringified-array, `^`-prefixed single, unresolvable-errors, malformed-scalar-errors-without-clearing. RED before fix, GREEN after.
- [x] `cargo nextest run -p swissarmyhammer-kanban` (1504 passed) `&& cargo nextest run -p swissarmyhammer-tools` (1255 passed). RED→GREEN confirmed.
- [x] `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools -- -D warnings` clean; `cargo build` workspace clean.

## Follow-up note (not blocking this task)
After merging, rebuild/reinstall the served `kanban` MCP binary (kanban-cli serve / Kanban.app sidecar) so the running tool picks up the fix.

## Workflow
- Used `/tdd` — wrote failing real-path e2e (single-string / stringified-array shapes) first, watched it fail through `KanbanTool::execute`, then made `resolve_depends_on` forgiving. #bug

## Review Findings (2026-06-17 08:35)

Verdict: CLEAN — all acceptance criteria verified, no real findings. Moved to done.

- [x] Real-path requirement met: served-tool e2e tests (`test_depends_on_persists_across_input_shapes_via_served_tool`, `test_add_task_depends_on_single_string_persists_via_served_tool`, `test_depends_on_unresolvable_ref_errors_via_served_tool` in `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`) genuinely drive `KanbanTool::execute()` and round-trip via a real `get task` read — they do NOT bypass to `parse_input`/`execute_operation`. Gap closed.
- [x] Shapes-matrix confirmed: 5 id formats (full ULID, 7-char short, `^short`, 12-char prefix, lowercase) × 3 wire shapes (single string, JSON array, stringified array) = 15 cases, each asserting `deps[0] == full ULID`.
- [x] RED→GREEN is genuine: pre-fix `resolve_depends_on` gated on `.and_then(|v| v.as_array())` and returned `Ok(None)` for any non-array — so the single-string and stringified-array cases would have silently dropped to `[]`. Verified the old code via `git show HEAD`. All 15 served-tool + 12 dispatch/roundtrip tests pass GREEN (re-ran 2026-06-17).
- [x] Malformed-scalar behavior is sound: `depends_on_refs` errors on a non-string/non-array value; the absent-param path is structurally separate (`resolve_depends_on` returns `Ok(None)` only when the param is missing, so `with_depends_on` is never called and existing deps are untouched). `dispatch_update_task_depends_on_malformed_scalar_errors_without_clearing` seeds a dep, sends `42`, asserts error + dep survives. No regression to "absent leaves deps unchanged".
- [x] Docs (AC #6): both `AddTask`/`UpdateTask` `depends_on` field docstrings and `description.md` state `depends_on` is the input (any format/shape) and `blocked_by` is the derived/computed (not directly settable) field — `blocked_by` is in fact set only by `task_helpers::enrich_task`, never a settable struct field.

### Refuted (false positive)
- [x] ~~Engine nit: `add.rs:31` / `update.rs:21` docstring references `blocked_by` as a field that doesn't exist on the struct — "remove or clarify".~~ FALSE POSITIVE. The line numbers are wrong (the `blocked_by` mention is on the `depends_on` field docstring, ~add.rs:40 / update.rs:39), and the docstring does NOT claim `blocked_by` is a struct field — it correctly documents `blocked_by` as the DERIVED projection (computed in `task_helpers::enrich_task`) to contrast it with the settable `depends_on` input. This is intentional and directly satisfies AC #6.