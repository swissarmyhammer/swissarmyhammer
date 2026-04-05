---
assignees:
- claude-code
depends_on:
- 01KND4KPNAEPKSDEN98TFYBEH1
position_column: todo
position_ordinal: '8680'
title: 'VT-7: Strategy — BLOCKING virtual tag'
---
## What

Implement the BLOCKING virtual tag strategy. A task is BLOCKING when at least one other task depends on it AND this task is NOT yet in the terminal column.

**Strategy metadata:**
- `slug()` → `\"BLOCKING\"`
- `color()` → `\"d73a4a\"` (red — signals urgent/critical path)
- `description()` → `\"Other tasks depend on this one\"`
- `commands()` → \"Show Dependents\" (navigate to/highlight dependent tasks)

**Files to modify:**
- `swissarmyhammer-kanban/src/virtual_tags.rs` — add `BlockingStrategy` struct implementing `VirtualTagStrategy`
  - `matches()` → reuse logic from `task_blocks()` — true if blocks list is non-empty AND task is not in terminal column
- Register in `default_virtual_tag_registry()`
- Implement backend command handler for \"Show Dependents\"

## Acceptance Criteria
- [ ] `BlockingStrategy` implements `VirtualTagStrategy` with all methods including commands
- [ ] Task depended on by others AND not complete → has BLOCKING tag
- [ ] Task depended on by others BUT already complete → does NOT have BLOCKING tag
- [ ] Task not depended on → does NOT have BLOCKING tag
- [ ] \"Show Dependents\" command declared and handler implemented
- [ ] Registered in default registry

## Tests
- [ ] Unit test: task with dependents not in terminal matches BLOCKING
- [ ] Unit test: completed blocker task does not match BLOCKING
- [ ] Unit test: task with no dependents does not match BLOCKING
- [ ] Unit test: commands() includes \"Show Dependents\"
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags