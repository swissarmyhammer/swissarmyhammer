---
assignees:
- claude-code
position_column: todo
position_ordinal: '8980'
title: Remove global activity log (current.jsonl) and list activity operation
---
## What

Remove the `.kanban/activity/current.jsonl` global activity log and the `list activity` kanban operation. Per-entity JSONL logs (per-task, per-actor, etc.) remain — only the global rollup is being removed.

### Files to modify

- `swissarmyhammer-kanban/src/processor.rs` — Remove the `write_log()` method and the call to `ctx.append_activity()` in `process()` (~lines 75-104)
- `swissarmyhammer-kanban/src/context.rs` — Remove `activity_dir()`, `activity_path()`, `append_activity()`, `read_activity()` methods. Remove `activity_dir().exists()` check from `directories_exist()` (~line 313). Remove `create_dir_all(self.activity_dir())` from `create_directories()` (~line 331). Remove related unit tests (~lines 1147-1357)
- `swissarmyhammer-kanban/src/activity/list.rs` — Delete this file entirely (ListActivity operation)
- `swissarmyhammer-kanban/src/activity/mod.rs` — Remove or delete the activity module (if list.rs was the only thing in it)
- `swissarmyhammer-kanban/src/dispatch.rs` — Remove ListActivity from dispatch registration
- `swissarmyhammer-kanban/src/schema.rs` — Remove `list activity` from schema definitions
- `swissarmyhammer-kanban/src/lib.rs` — Remove `activity/current.jsonl` from the doc comment directory tree (~line 57), remove `mod activity` if module deleted
- `swissarmyhammer-kanban/tests/integration_logging.rs` — Remove or rewrite tests that assert on `read_activity()` / `activity_path()`
- `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — Remove test asserting `activity_dir().exists()` (~line 1995), remove `list activity` from MCP tool schema/enum
- `swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs` — Remove `list activity` from registration assertions
- `kanban-app/src/state.rs` — Remove `create_dir_all(kanban_dir.join("activity"))` (~line 974)

## Acceptance Criteria

- [ ] No code references `activity_dir`, `activity_path`, `append_activity`, or `read_activity`
- [ ] No code references `ListActivity` or dispatches `list activity`
- [ ] `cargo build` succeeds with no errors
- [ ] `cargo test` passes — all remaining tests green
- [ ] Running kanban operations (add task, move task, etc.) no longer creates or appends to `.kanban/activity/current.jsonl`
- [ ] The `list activity` operation is removed from the MCP tool schema enum

## Tests

- [ ] `cargo test -p swissarmyhammer-kanban` — all tests pass after removing activity-related tests
- [ ] `cargo test -p swissarmyhammer-tools` — MCP tool tests pass without activity assertions
- [ ] `cargo test -p swissarmyhammer-cli` — registration tests pass without list activity
- [ ] `cargo test` (workspace-wide) — full green
- [ ] Manual: run `sah kanban add task --title "test"` and confirm no `activity/current.jsonl` is created