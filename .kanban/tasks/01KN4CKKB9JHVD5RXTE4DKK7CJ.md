---
assignees:
- claude-code
depends_on:
- 01KN4CJX3PD2A1RTVAGBZKX7MH
- 01KN4CK12F6Y8NCJX186EF0003
- 01KN4CK3YAFB7E4RDS179E9X40
position_column: done
position_ordinal: ffffffffffffffffffb780
title: 'EXTRACT-5: Rewire kanban to depend on new crate'
---
## What

Update `swissarmyhammer-kanban` to depend on the new `swissarmyhammer-perspectives` crate instead of owning the domain types.

**`swissarmyhammer-kanban/Cargo.toml`:**
- Add `swissarmyhammer-perspectives = { path = "../swissarmyhammer-perspectives" }`

**Delete from kanban** (already moved to new crate):
- `src/perspective/types.rs`
- `src/perspective/context.rs`
- `src/perspective/changelog.rs`

**`src/perspective/mod.rs`:**
- Remove `pub mod types;`, `pub mod context;`, `pub mod changelog;`
- Re-export from new crate: `pub use swissarmyhammer_perspectives::{Perspective, PerspectiveFieldEntry, SortDirection, SortEntry, PerspectiveContext, PerspectiveChangelog, PerspectiveChangeEntry, PerspectiveChangeOp};`

**`src/context.rs`:**
- Update imports of PerspectiveContext/PerspectiveChangelog to come from `swissarmyhammer_perspectives`

**`src/error.rs`:**
- Add `From<swissarmyhammer_perspectives::PerspectiveError> for KanbanError` impl

**Operation files** (`add.rs`, `get.rs`, `list.rs`, `update.rs`, `delete.rs`):
- Update imports to use types from `swissarmyhammer_perspectives` (or via the re-exports in perspective/mod.rs)

**`src/dispatch.rs`:**
- Update perspective type imports

**`src/commands/perspective_commands.rs`:**
- Update perspective type imports

## Acceptance Criteria
- [ ] `cargo check -p swissarmyhammer-kanban` compiles
- [ ] No `swissarmyhammer-kanban` code defines Perspective/PerspectiveContext/PerspectiveChangelog — all imported from new crate
- [ ] `From<PerspectiveError> for KanbanError` exists
- [ ] All kanban tests still pass

## Tests
- [ ] `cargo test -p swissarmyhammer-kanban` — all existing tests pass
- [ ] `cargo test -p swissarmyhammer-commands` — all pass
- [ ] `cargo test -p swissarmyhammer-perspectives` — all pass