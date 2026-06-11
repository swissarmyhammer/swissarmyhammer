---
assignees:
- claude-code
depends_on:
- 01KTRYRC9DSWX5X5X11624PRSF
position_column: todo
position_ordinal: '9880'
project: op-token-diet
title: Heavy task mutations return thin ack {ok,id,short_id}; add/paste return slim task
---
## What
Five task mutation ops in `crates/swissarmyhammer-kanban/src/task/` end their `execute` with `Ok(task_entity_to_json(&entity))` (`crates/swissarmyhammer-kanban/src/task_helpers.rs`), echoing the FULL task — `description`, `attachments`, every field — back to the agent that already has it. An `update task` on a long card echoes kilobytes per call, every call. Replace those responses:

**Pure identity acknowledgment** — `update.rs` (`UpdateTask`), `mv.rs` (`MoveTask`), `complete.rs` (`CompleteTask`) return:
```json
{ "ok": true, "id": "<ulid>", "short_id": "<7-char>" }
```
- NO `changed` field map (DECIDED — rejected): echoing changed-field VALUES re-pays for the same tokens the agent just sent — an updated description/body would be billed twice. Server-computed values (normalized `YYYY-MM-DD` dates from `apply_optional_date`, move/complete ordinals) are not worth a special case either: the agent asked for the move/date, success implies it took effect, and `get task` is the escape hatch on the rare occasion the stored value matters.
- The mutation either fully succeeds (ack) or errors (KanbanError) — there is no partial-success state the response needs to describe.

**Slim projection for creations** — `add.rs` (`AddTask`) and `paste.rs` (`PasteTask`) return `slim_task_json(...)` (the allowlist projection introduced by card ^624prsf): the agent needs `id`/`short_id`/`position` it didn't have, but not its own description/attachments echoed back.

Implementation:
- Add `pub(crate) fn task_mutation_ack(entity: &Entity) -> Value` in `crates/swissarmyhammer-kanban/src/task_helpers.rs` next to `task_entity_to_json` — single source of the envelope; derives `short_id` via `crate::types::short_id`.
- HARD CONSTRAINT: top-level `id` must remain — the MCP wrapper `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` reads `result["id"]` (in `KanbanTool::call`, near `should_include_plan`) to set `_plan._meta.affected_task_id`. The `_plan` attachment itself is at the tools layer and is untouched by this card.
- `get task` (`get.rs`) and `next task` (`next.rs`) stay FULL (`task_entity_to_rich_json`) — the agreed escape hatch; want the full card after mutating → call `get task`.
- NO `detail` param on mutations (decided). Alternatives weighed and rejected: (a) slim projection on every mutation — still echoes ~15 fields the agent already knows; (b) `detail` param defaulting to thin — extra API surface duplicating `get task`; (c) `changed` field/value map — re-echoes the very payload the agent just sent (see above).
- Consumers verified: kanban-cli (`apps/kanban-cli/src/main.rs::execute_kanban_operation`, `commands/serve.rs`) prints whatever YAML/JSON comes back — no field assumptions; desktop UI consumes broadcast events through the entity store, never op responses; skill prose (builtin/skills/plan/references/PLANNING_GUIDE.md) only captures the returned task id from `add task`, which the slim projection keeps.

Test migration (the real blast radius — tests asserting the old full shape on mutation responses):
- `crates/swissarmyhammer-kanban/src/dispatch.rs` `#[cfg(test)]`: asserts like `result["title"]`, `result["description"]`, `result["position"]["column"]` directly on add/update/move/complete results (e.g. update→"New desc", move→position asserts). Re-point shape asserts at the ack/slim shape; where a test needs to verify the mutation's EFFECT (new title, new position, normalized date), fetch via `get task` and assert there — that's also the more honest test (asserts stored state, not response echo).
- Per-op `#[cfg(test)]` modules: `add.rs` (asserts `result["description"]`, system-date checks), `update.rs`, `mv.rs`, `complete.rs`, `paste.rs`.
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` tests: asserts on `data["title"]`/`data["position"]["column"]` after add/update/move/complete.

## Acceptance Criteria
- [ ] `update task`, `move task`, `complete task` responses contain exactly `ok`, `id`, `short_id` — nothing else; no field echo of any kind.
- [ ] `add task` / `paste` return the slim projection (allowlist fields incl. `id`, `short_id`, `position`; NO `description`/`attachments` keys).
- [ ] Mutation effects are still fully verified: tests assert post-op state via `get task` (e.g. update title → get task shows new title; move → get task shows new column/ordinal; due date → normalized `YYYY-MM-DD`).
- [ ] MCP kanban tool still attaches `_plan` with `affected_task_id` populated from mutation responses.
- [ ] `get task` and `next task` responses unchanged (full rich JSON, description present).
- [ ] `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools -- -D warnings` clean.

## Tests
- [ ] `task_helpers.rs` unit test for `task_mutation_ack`: envelope has exactly ok/id/short_id; short_id derived from the ULID.
- [ ] Op tests (TempDir board pattern: `InitBoard` + `AddTask`) in `update.rs`, `mv.rs`, `complete.rs`: assert the exact three-key response set and the absence of `description`/`title`/`position`; then `get task` asserts the stored effect. `add.rs`/`paste.rs`: response is slim (no `description`/`attachments` keys, has `position`).
- [ ] `dispatch.rs` integration tests migrated: add → update(title) asserts ack shape; effect asserts go through `get task`.
- [ ] `crates/swissarmyhammer-tools` kanban tool test: `update task` response carries `_plan._meta.affected_task_id` equal to the mutated task id.
- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-tools` — green.

## Workflow
- Use `/tdd` — write the ack-shape tests first (red), then implement `task_mutation_ack` and rewire the five ops.