---
comments:
- actor: wballard
  id: 01kvp0yyb95armkbcanxmx9vfv
  text: 'Implemented in parse/mod.rs `normalize_params` + `infer_operation`. Verified: 3 new parse tests pass, dispatch real-path test `dispatch_get_task_accepts_task_key_alias` passes (full ULID + `^<short>` under the `task` key), `cargo fmt --check` and `cargo clippy --lib --tests -D warnings` clean. (This very comment hit the bug on the running pre-fix MCP server: `add comment {task: ...}` failed with `missing required field: task_id` — exactly what the member-op `task`→`task_id` alias fixes.)'
  timestamp: 2026-06-21T21:21:37.641029+00:00
position_column: review
position_ordinal: '8180'
title: 'kanban tool: accept `task` as alias for `id` on task ops'
---
## Problem

Agents (clustering in the committer role) call `kanban get task` with the task reference under the key `task`:

```
ERR  {"op": "get task", "task": "^1hcfjkm"}
     -> MCP error -32603: get task: parse error: missing required field: id
OK   {"op": "get task", "id":   "^1hcfjkm"}
```

The model emits the wrong key, eats a parse error, then retries the identical reference under `id` — pure waste, a full round-trip every time, and it recurs. The short-id form itself is fine; it's purely a parameter-name mismatch (`task` vs `id`).

## Fix

Forgiving-input alias in `crates/swissarmyhammer-kanban/src/parse/mod.rs::normalize_params` (the kanban tool already documents alias-tolerance for `depends_on`):

- Task-level ops: `task` → `id` (joins existing `taskId`/`task_id` → `id` group). An explicit `id` already present is not clobbered.
- Member ops (comment/attachment): `task` → `task_id` (owning task), never the member's own `id`.
- `infer_operation`: recognize a bare `{ "task": "x" }` as id-bearing so it infers `get task`.

Verified no operation struct uses `task` as a deserialize field (all source matches are entity-type literals / locals), so no collision.

## Tests

Added to `parse/mod.rs`:
- `test_task_aliases_to_id_on_task_level_ops`
- `test_bare_task_key_infers_get_task`
- `test_task_aliases_to_task_id_on_member_ops`