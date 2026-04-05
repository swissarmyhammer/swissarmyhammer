---
assignees:
- claude-code
depends_on:
- 01KND4KPNAEPKSDEN98TFYBEH1
position_column: todo
position_ordinal: '8480'
title: 'VT-5: Strategy — READY virtual tag'
---
## What

Implement the READY virtual tag strategy. A task is READY when all its dependencies are complete (in the terminal column) or it has no dependencies at all. Tasks already in the terminal column do NOT get READY.

**Strategy metadata:**
- `slug()` → `\"READY\"`
- `color()` → `\"0e8a16\"` (green — signals go)
- `description()` → `\"Task has no unmet dependencies\"`
- `commands()` → TBD — possible actions: \"Start Working\" (move to doing column)

**Files to modify:**
- `swissarmyhammer-kanban/src/virtual_tags.rs` — add `ReadyStrategy` struct implementing `VirtualTagStrategy`
  - `matches()` → reuse logic from `task_is_ready()` in task_helpers.rs, plus exclude terminal column tasks
- Register in `default_virtual_tag_registry()`
- If declaring commands: implement the backend command handler (e.g. `vtag.ready.start` in `src/virtual_tag_commands/` or similar)

## Acceptance Criteria
- [ ] `ReadyStrategy` implements `VirtualTagStrategy` with all methods
- [ ] Task with no deps and not in terminal column → has READY tag
- [ ] Task with all deps complete and not in terminal → has READY tag
- [ ] Task with unmet deps → does NOT have READY tag
- [ ] Task in terminal column → does NOT have READY tag
- [ ] Registered in default registry
- [ ] Commands declared and backend handlers implemented

## Tests
- [ ] Unit test: task with no deps matches READY
- [ ] Unit test: task with completed deps matches READY
- [ ] Unit test: task with incomplete dep does not match READY
- [ ] Unit test: completed task does not match READY
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags