---
assignees:
- claude-code
depends_on:
- 01KTV7WVTKX8BPK8VGGH472NDJ
position_column: todo
position_ordinal: '9980'
project: op-token-diet
title: Normalize remaining task-mutation acks to the standard envelope (uniform top-level id)
---
## What
The task mutations that are already thin use ad-hoc ack shapes in `crates/swissarmyhammer-kanban/src/task/`: `tag.rs` returns `{"tagged": true, "task_id": ...}`, `assign.rs` returns `{"assigned": true, "task_id", "assignee", "all_assignees"}`, similarly `untag.rs`, `unassign.rs`, `delete.rs`, `cut.rs`, `copy.rs`, and `archive.rs` (ArchiveTask/UnarchiveTask). Because they expose `task_id` instead of `id`, the MCP wrapper's `_plan` affected-task extraction (`result["id"]` in `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`) silently misses these ops today — `_plan._meta.affected_task_id` is never set for `tag task`/`assign task`/etc. Converge them on the pure identity envelope introduced by the heavy-mutations card (^h472ndj, `task_mutation_ack` in `crates/swissarmyhammer-kanban/src/task_helpers.rs`):

- `tag.rs` / `untag.rs` / `assign.rs` / `unassign.rs`: `{ok, id, short_id}` — nothing else. NO `changed` map, no `tags`/`all_assignees` echo (DECIDED on ^h472ndj: mutations acknowledge, they don't echo; success implies the tag/assignee took effect; `get task` is the escape hatch). The ad-hoc `tagged`/`assigned`/`assignee`/`all_assignees`/`task_id` keys are removed.
- `delete.rs`, `archive.rs` (archive + unarchive): standardize identity keys plus the op-specific flag: `{ok, id, short_id, "deleted": true}` / `{..., "archived": true}` / `{..., "unarchived": true}`.
- `cut.rs` / `copy.rs`: standardize `id`/`short_id` keys; keep their clipboard-specific payload keys as-is (the clipboard payload IS the information the agent needs back).

Scope decisions (deliberate):
- Non-task entity mutations (actor/tag/project/column/attachment add/update/delete ops) are OUT of scope — their entities are tens of bytes; no token problem, no churn justified.
- `list archived` (`archive.rs::ListArchived`) is a READ op — its slim-default treatment is in scope of the list-slim card (^624prsf), not changed by this card.

Test migration: per-op `#[cfg(test)]` modules in the files above plus `crates/swissarmyhammer-kanban/src/dispatch.rs` asserts on `tagged`/`assigned`/`task_id`/`all_assignees`; grep for those keys across `crates/` and `apps/` to catch stragglers. Where a test verified the post-op tag/assignee list off the response, re-point it at `get task` (asserts stored state, not response echo).

## Acceptance Criteria
- [ ] `tag task`, `untag task`, `assign task`, `unassign task` responses contain exactly `ok`, `id`, `short_id`; no `task_id`/`tagged`/`assigned`/`assignee`/`all_assignees` keys remain.
- [ ] `delete task`, `archive task`, `unarchive task`, `cut`, `copy` responses carry top-level `id` and `short_id`.
- [ ] MCP kanban tool sets `_plan._meta.affected_task_id` for `tag task` and `assign task` (previously missed).
- [ ] `rg '"task_id"' crates/swissarmyhammer-kanban/src/task/` returns no response-construction hits.
- [ ] Post-op effects verified via `get task` in tests (tag present, assignee present), not via response echo.
- [ ] `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools -- -D warnings` clean.

## Tests
- [ ] Op tests (TempDir board pattern) in `tag.rs`, `untag.rs`, `assign.rs`, `unassign.rs`: exact three-key response set; then `get task` asserts the post-op tag/assignee list.
- [ ] `delete.rs`/`archive.rs`/`cut.rs`/`copy.rs` tests: `id` + `short_id` present, op-specific flag/payload intact.
- [ ] `crates/swissarmyhammer-tools` kanban tool test: `tag task` response carries `_plan._meta.affected_task_id` equal to the tagged task id (regression for the previously-missed extraction).
- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-tools` — green.

## Workflow
- Use `/tdd` — write the envelope-shape tests first (red), then rewire the acks onto `task_mutation_ack`.