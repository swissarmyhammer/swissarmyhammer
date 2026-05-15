---
assignees:
- claude-code
depends_on:
- 01KNJD76746DTJ540A5SHGRCF0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffb680
position_swimlane: null
title: 'FILTER-1: Wire filter-expr validation into SetFilterCmd'
---
## What

Integrate `swissarmyhammer-filter-expr` into the perspective commands so filter expressions are validated on save. Currently `SetFilterCmd` stores the filter string verbatim — it should now parse it and reject invalid expressions with a clear error.

### Files to modify
- `swissarmyhammer-kanban/Cargo.toml` — add dep on `swissarmyhammer-filter-expr`
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — `SetFilterCmd::execute()` calls `filter_expr::parse()` before storing; returns `CommandError::ExecutionFailed` with parse error details on failure
- `swissarmyhammer-kanban/tests/perspective_integration.rs` — add integration tests

### Behavior
- Valid DSL expressions (e.g. `#bug && @will`) are stored as-is
- Invalid expressions return an error with the parse error message
- Empty/cleared filters still work (ClearFilterCmd unchanged)
- The `SavePerspectiveCmd` also validates if a filter arg is provided
- No backward compatibility with old JS expressions — they are rejected as invalid

## Acceptance Criteria
- [ ] `perspective.filter` with `#bug && @will` succeeds and stores the expression
- [ ] `perspective.filter` with `invalid $$$ garbage` returns a CommandError with parse details
- [ ] `perspective.filter` with old JS like `Status !== "Done"` is rejected as invalid
- [ ] `perspective.clearFilter` still works unchanged

## Tests
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — unit test: valid DSL accepted
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — unit test: invalid expression rejected with error
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — unit test: old JS expression rejected
- [ ] `swissarmyhammer-kanban/tests/perspective_integration.rs` — integration test: round-trip save/load with DSL filter
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.