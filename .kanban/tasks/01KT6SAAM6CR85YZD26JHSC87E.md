---
assignees:
- claude-code
depends_on:
- 01KT6R6HR3KJT6JVNDRAJV8V4T
position_column: todo
position_ordinal: '8280'
project: short-ids
title: 'Short IDs: tool/CLI API accepts short id as input + emits it in output'
---
Make the short id usable from the kanban tool/MCP and CLI surfaces. Input is forgiving (short or full ULID); storage stays canonical (full ULID); output exposes the short id.

## Scope
- Input — id args: anywhere the kanban operations accept a task id (get/move/complete/assign/unassign/tag/untag/update/delete/archive), also accept the 7-char short id or a `^<short>` form, resolved via the core resolver to the full ULID. Case-insensitive. Full ULID continues to work unchanged.
  - Insertion point: the id-coercion in `crates/swissarmyhammer-kanban/src/dispatch.rs` (`TaskId::from_string(...)` call sites) — route task-id args through the resolver instead of a raw `from_string`.
- Input — structured ref fields: `depends_on` (and any future task-ref field) accepts short ids on write, but NORMALIZES them to the full ULID before storing. The stored value is always the full 26-char ULID (see Storage policy on the core card).
- Output: include a derived `short_id` field on the task JSON returned by the tool, and render `^<short>` in any human-formatted CLI output (kanban-cli). Stored JSONL is unchanged.

## Acceptance
- A task can be fetched/moved/completed by `^8rfp1r`, by `8rfp1r`, and by full ULID; case-insensitive.
- `add task`/`update task` with `depends_on` given as short ids persists full ULIDs in the JSONL.
- Tool task JSON includes `short_id`.
- An unknown/ambiguous short id returns a clean not-found error, not a panic.

Depends on core derivation/resolver.