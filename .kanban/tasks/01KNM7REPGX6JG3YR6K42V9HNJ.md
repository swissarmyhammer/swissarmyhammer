---
assignees:
- claude-code
depends_on:
- 01KNM7QS0FJ0JNRYJD3NG3BWHC
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9880
title: 'MCP/CLI: expose user-set date fields in task operations'
---
## What

Update the MCP kanban tool and CLI commands to support the new date fields:

1. **`add task`** — accept optional `due` and `scheduled` parameters (ISO 8601 date strings). Set them as entity fields before write.

2. **`update task`** — accept optional `due` and `scheduled` parameters. Allow clearing with null/empty string.

3. **`get task`** / **`list tasks`** — include all date fields in task output. User-set dates (due, scheduled) come from stored fields. System dates (created, updated, started, completed) come from computed field derivation (already happens via the enrichment pipeline on read).

4. System dates are read-only — reject attempts to set created/updated/started/completed via add/update.

**Files to modify:**
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — add date params to AddTask/UpdateTask structs; include dates in task JSON output
- `swissarmyhammer-kanban/src/task/add.rs` — wire `due`/`scheduled` params through to entity.set()
- Update task command similarly

## Acceptance Criteria
- [x] `add task` accepts `due` and `scheduled` ISO 8601 date strings
- [x] `update task` accepts `due` and `scheduled`, including clearing them
- [x] `get task` returns all date fields (user-set + system-derived) when present
- [x] `list tasks` returns date fields on each task
- [x] Invalid date strings are rejected with clear error
- [x] System dates cannot be set via MCP/CLI params

## Tests
- [x] Integration test: add task with due date → get task → verify due date returned
- [x] Integration test: update task to set/clear scheduled date
- [x] Integration test: create task → get task → verify created/updated computed dates appear
- [x] Integration test: move task through columns → verify started/completed appear
- [x] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates

## Review Findings (2026-04-12 18:30)

### Warnings
- [x] `swissarmyhammer-kanban/src/task_helpers.rs:485` — `auto_create_body_tags` here is now dead code. This card migrated the add/update callers to the new `task::shared::auto_create_body_tags` (reversed arg order, different return type), but left the original function in `task_helpers.rs` behind. Two near-duplicate implementations of the same concept now exist, and the older one has zero callers. Delete `task_helpers::auto_create_body_tags` (and its internal helpers if they become unused) so only the `task::shared` version remains.

### Nits
- [x] `swissarmyhammer-kanban/src/dispatch.rs:242` — `date_param_to_update` treats whitespace-only strings (`"  "`) as "clear", but the Rust builder `UpdateTask::with_due("  ")` flows through `apply_optional_date` which uses `raw.is_empty()` (no trim) and therefore rejects it via `parse_iso8601_date`. Make the two paths consistent — either also trim in `apply_optional_date` or stop trimming in `date_param_to_update` so MCP and Rust callers see identical semantics.
- [x] `swissarmyhammer-kanban/src/dispatch.rs:178` — `dispatch_add_task` uses `op.get_string("due")` which silently drops non-string JSON values (e.g. an integer `42`), whereas `dispatch_update_task` routes them through `date_param_to_update` so they produce a clear downstream error. Add gets the softer, harder-to-debug behavior. Consider reusing a single helper so non-string values on `add task` also error out with a useful message.
- [x] `swissarmyhammer-kanban/src/context.rs:607` — `is_safe_entity_type` is new and load-bearing (it guards store registration against traversal via a malformed local YAML entity name), but has no direct unit tests. Add a small test covering empty / leading-dot / `..` / forward-slash / back-slash / embedded-`..` cases so future refactors can't weaken it silently.
- [x] `swissarmyhammer-kanban/src/task/add.rs:504` — `test_add_task_does_not_accept_system_date_params` only asserts `created != "1999-01-01"`. That catches a flagrant write but not a subtler one where a system-date param gets written to a differently-named internal field. Strengthen by asserting directly on the entity storage (or by also checking `updated/started/completed` against the injected value).

## Review Resolution (2026-04-12)

All five review findings addressed:

- **Warning (dead code)**: Removed `task_helpers::auto_create_body_tags`; the canonical implementation in `task::shared` is now the only one.
- **Nit — whitespace-only clear consistency**: `apply_optional_date` in `task/update.rs` now treats whitespace-only input as a clear, matching `date_param_to_update`. `parse_iso8601_date` also trims internally so any stray whitespace produces a consistent "empty string" error across add/update. Added regression test `test_update_task_whitespace_only_date_clears_via_builder`.
- **Nit — non-string date values on add**: Added `date_param_to_add` helper in `dispatch.rs` that coerces non-string, non-null JSON values to strings and forwards them through the existing date parser so callers get a clear error instead of silent drops. Added regression tests `dispatch_add_task_rejects_non_string_date` and `dispatch_add_task_rejects_non_string_scheduled`.
- **Nit — `is_safe_entity_type` coverage**: Added 7 unit tests in `context.rs` covering normal names, empty, leading dot (and bare `.`), `..`, forward/back slashes, and embedded `..` substrings (leading/middle/trailing).
- **Nit — stronger system-date assertion**: `test_add_task_does_not_accept_system_date_params` now (a) checks all four date output fields against the sentinel and (b) scans the raw stored entity's field map for the sentinel to catch misrouted writes under a different field name.

All workspace tests pass with zero failures and zero warnings.