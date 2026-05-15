---
assignees:
- claude-code
depends_on:
- 01KN4CJJPSRQNEB17E6CQFF0PH
position_column: done
position_ordinal: ffffffffffffffffffb680
title: 'EXTRACT-4: Move changelog.rs to new crate'
---
## What

Move `swissarmyhammer-kanban/src/perspective/changelog.rs` to `swissarmyhammer-perspectives/src/changelog.rs`.

**Changes needed:**
- Replace any `KanbanError` references with `PerspectiveError` (if any)
- Update imports to use `crate::types::Perspective`
- Update imports to use `crate::error::Result` if needed

Move all changelog unit tests along.

## Acceptance Criteria
- [x] `perspectives/src/changelog.rs` compiles standalone
- [x] All changelog tests pass in new crate
- [x] No references to `swissarmyhammer_kanban` remain

## Tests
- [x] All existing changelog tests pass in new location
- [x] `cargo test -p swissarmyhammer-perspectives changelog`