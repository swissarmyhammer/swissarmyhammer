---
assignees:
- claude-code
depends_on:
- 01KNJD8P8KPJK4HN8JCDKEH724
- 01KNKXQNDXDDSPWNW5GGAYCVRC
- 01KNJDBNQ0QBJZYCW1AGSP96SG
position_column: done
position_ordinal: ffffffffffffffffffffff9680
title: 'FILTER-8: End-to-end integration test scenarios'
---
## What

End-to-end integration tests that exercise the full filter pipeline: DSL expression → Rust parser → evaluator → filtered results, across all three surfaces (Tauri `list_entities`, MCP `list tasks`, and perspective save/load). These tests verify that the Rust parser and Lezer grammar agree on what's valid, and that real board data flows correctly through filtering.

### Scenarios

**Basic filtering**
1. Create board with tasks tagged #bug, #feature, #docs. Filter `#bug` → only bug tasks returned.
2. Create tasks assigned to @alice and @bob. Filter `@alice` → only Alice's tasks.
3. Create tasks with dependencies. Filter `^TASK-ID` → only tasks referencing that ID.

**Boolean logic**
4. Filter `#bug && @alice` → tasks that are both tagged bug AND assigned to Alice.
5. Filter `#bug || #feature` → tasks tagged bug OR feature (not docs).
6. Filter `!#done` → tasks NOT tagged done.
7. Filter `(#bug || #feature) && @alice` → grouping works correctly.
8. Filter `#bug @alice` → implicit AND behaves same as `#bug && @alice`.

**Virtual tags**
9. Create tasks with dependencies (some satisfied, some not). Filter `#READY` → only tasks whose deps are all complete.
10. Filter `#BLOCKED` → only tasks with unsatisfied deps.
11. Filter `#READY && #bug` → ready bug tasks only.

**Keyword operators**
12. Filter `not #done and @alice or #urgent` → keyword operators parse and evaluate correctly.
13. Filter `NOT #done AND @alice` → uppercase keywords work.

**Edge cases**
14. Empty filter string → all tasks returned (no filtering).
15. Invalid filter `$$garbage` → error returned.
16. Filter with no matches `#nonexistent-tag` → empty result set, not an error.
17. Tag names with hyphens and dots: `#v2.0`, `#bug-fix` → work correctly.

**Cross-surface consistency**
18. Same filter expression produces identical results via `list_entities` (Tauri) and `ListTasks` (MCP/CLI).
19. Save a perspective with a filter, reload it, verify the filter string round-trips and evaluates correctly.

### Files to create/modify
- `swissarmyhammer-kanban/tests/filter_integration.rs` — new integration test file with all scenarios above
- May also add scenarios to `swissarmyhammer-kanban/tests/perspective_integration.rs` for perspective-specific flows

### Test structure
Each scenario: set up board state (init board, add tasks with tags/assignees/deps), apply filter, assert correct subset returned. Use the existing `KanbanContext` + `Execute` pattern from other integration tests.

## Acceptance Criteria
- [ ] All 19 scenarios pass
- [ ] Tests use real `KanbanContext` with temp directories (no mocks)
- [ ] Virtual tag scenarios create real dependency chains
- [ ] Cross-surface test proves Tauri and MCP paths return same results for same filter
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Tests
- [ ] `swissarmyhammer-kanban/tests/filter_integration.rs` — all scenarios listed above
- [ ] `cargo test -p swissarmyhammer-kanban --test filter_integration` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.