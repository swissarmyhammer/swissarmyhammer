---
assignees:
- claude-code
depends_on:
- 01KNF8S3X6BZX4YXVSWAD5FZSG
position_column: todo
position_ordinal: '8280'
title: 'Rust: Remove swimlane module and all references'
---
## What

Remove the entire swimlane concept from the Rust codebase AND the YAML definitions. Replace `swimlanes` with `projects` in the board response. This is a deletion-heavy card.

### Files to delete:
- `swissarmyhammer-kanban/src/swimlane/mod.rs`
- `swissarmyhammer-kanban/src/swimlane/add.rs`
- `swissarmyhammer-kanban/src/swimlane/get.rs`
- `swissarmyhammer-kanban/src/swimlane/update.rs`
- `swissarmyhammer-kanban/src/swimlane/delete.rs`
- `swissarmyhammer-kanban/src/swimlane/list.rs`
- `swissarmyhammer-kanban/builtin/fields/entities/swimlane.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/position_swimlane.yaml`

### Files to modify:
- **`swissarmyhammer-kanban/builtin/fields/entities/task.yaml`** — remove `position_swimlane` from fields list (was kept transitionally in Card 1)
- **`swissarmyhammer-kanban/src/lib.rs`** — remove `mod swimlane` and its re-exports
- **`swissarmyhammer-kanban/src/types/ids.rs`** — remove `define_id!(SwimlaneId, ...)`
- **`swissarmyhammer-kanban/src/types/mod.rs`** — remove `SwimlaneId` re-export
- **`swissarmyhammer-kanban/src/types/operation.rs`** — remove `Noun::Swimlane`, `Noun::Swimlanes` and their match arms
- **`swissarmyhammer-kanban/src/types/position.rs`** — remove `swimlane: Option<SwimlaneId>` from `Position`, simplify to `column + ordinal` only
- **`swissarmyhammer-kanban/src/error.rs`** — remove `SwimlaneNotFound`, `SwimlaneHasTasks` variants
- **`swissarmyhammer-kanban/src/dispatch.rs`** — remove all swimlane dispatch routes and imports; remove swimlane from task dispatch args
- **`swissarmyhammer-kanban/src/task/add.rs`** — remove `swimlane` field from `AddTask`
- **`swissarmyhammer-kanban/src/task/mv.rs`** — remove `swimlane` from `MoveTask`, remove `with_swimlane` and `to_column_and_swimlane`
- **`swissarmyhammer-kanban/src/task/update.rs`** — remove `swimlane` field
- **`swissarmyhammer-kanban/src/task/next.rs`** — remove swimlane filter
- **`swissarmyhammer-kanban/src/task/list.rs`** — remove swimlane filter
- **`swissarmyhammer-kanban/src/task/paste.rs`** — remove swimlane handling
- **`swissarmyhammer-kanban/src/task/complete.rs`** — remove swimlane handling
- **`swissarmyhammer-kanban/src/task/cut.rs`** — remove swimlane handling
- **`swissarmyhammer-kanban/src/task_helpers.rs`** — remove swimlane references
- **`swissarmyhammer-kanban/src/board/get.rs`** — replace `swimlanes` collection with `projects` in board response
- **`swissarmyhammer-kanban/src/board/init.rs`** — remove swimlane directory creation if any
- **`swissarmyhammer-kanban/src/context.rs`** — remove swimlane references
- **`swissarmyhammer-kanban/src/schema.rs`** — remove swimlane references
- **`swissarmyhammer-kanban/src/scope_commands.rs`** — remove swimlane references
- **`swissarmyhammer-kanban/src/defaults.rs`** — remove swimlane defaults
- **`swissarmyhammer-kanban/src/parse/mod.rs`** — remove swimlane parsing
- **`swissarmyhammer-kanban/tests/command_dispatch_integration.rs`** — remove all swimlane tests

### Also check:
- `swissarmyhammer-entity/src/io.rs` and `watcher.rs` for swimlane references

## Acceptance Criteria
- [ ] No file in `swissarmyhammer-kanban/src/swimlane/` exists
- [ ] `swimlane.yaml` entity and `position_swimlane.yaml` field definition deleted
- [ ] `task.yaml` no longer lists `position_swimlane`
- [ ] `grep -r swimlane swissarmyhammer-kanban/src/` returns zero hits
- [ ] `Position` struct is `{ column, ordinal }` only
- [ ] `get board` response includes `projects` array instead of `swimlanes`
- [ ] `cargo build -p swissarmyhammer-kanban` compiles cleanly
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Tests
- [ ] All existing tests that reference swimlane are updated or removed
- [ ] `cargo test -p swissarmyhammer-kanban` passes
- [ ] `cargo test -p swissarmyhammer-entity` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.