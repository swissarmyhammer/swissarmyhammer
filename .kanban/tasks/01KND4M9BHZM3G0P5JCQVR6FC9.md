---
assignees:
- claude-code
depends_on:
- 01KND4KPNAEPKSDEN98TFYBEH1
position_column: todo
position_ordinal: '8580'
title: 'VT-6: Strategy — BLOCKED virtual tag'
---
## What

Implement the BLOCKED virtual tag strategy. A task is BLOCKED when it has at least one dependency that is NOT in the terminal column.

**Strategy metadata:**
- `slug()` → `\"BLOCKED\"`
- `color()` → `\"e36209\"` (orange — signals caution/wait)
- `description()` → `\"Task has unmet dependencies\"`
- `commands()` → \"Show Blockers\" (navigate to/highlight the blocking tasks)

**Files to modify:**
- `swissarmyhammer-kanban/src/virtual_tags.rs` — add `BlockedStrategy` struct implementing `VirtualTagStrategy`
  - `matches()` → reuse logic from `task_blocked_by()` — true if blocked_by is non-empty
- Register in `default_virtual_tag_registry()`
- Implement backend command handler for \"Show Blockers\"

## Acceptance Criteria
- [ ] `BlockedStrategy` implements `VirtualTagStrategy` with all methods including commands
- [ ] Task with unmet dep → has BLOCKED tag
- [ ] Task with no deps → does NOT have BLOCKED tag
- [ ] Task with all deps complete → does NOT have BLOCKED tag
- [ ] \"Show Blockers\" command declared and handler implemented
- [ ] Registered in default registry

## Tests
- [ ] Unit test: task with incomplete dep matches BLOCKED
- [ ] Unit test: task with no deps does not match BLOCKED
- [ ] Unit test: task with all deps complete does not match BLOCKED
- [ ] Unit test: commands() includes \"Show Blockers\"
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags