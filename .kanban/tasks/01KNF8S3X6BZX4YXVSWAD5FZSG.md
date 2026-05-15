---
assignees:
- claude-code
depends_on:
- 01KNF8RC3NRWS8H1G64RQMZFTA
position_column: done
position_ordinal: ffffffffffffffffffffffffffff8980
title: 'Rust: Create Project CRUD operations'
---
## What

Create the full Project CRUD module in Rust, mirroring the existing swimlane/column pattern.

### Files to create:
- **Create** `swissarmyhammer-kanban/src/project/mod.rs` — module with add, get, update, delete, list submodules
- **Create** `swissarmyhammer-kanban/src/project/add.rs` — AddProject operation (id, name, description, color, order)
- **Create** `swissarmyhammer-kanban/src/project/get.rs` — GetProject
- **Create** `swissarmyhammer-kanban/src/project/update.rs` — UpdateProject (name, description, color, order)
- **Create** `swissarmyhammer-kanban/src/project/delete.rs` — DeleteProject (fail if tasks reference it)
- **Create** `swissarmyhammer-kanban/src/project/list.rs` — ListProjects

### Files to modify:
- **Modify** `swissarmyhammer-kanban/src/types/ids.rs` — add `define_id!(ProjectId, "Identifier for projects (slug-style)")`
- **Modify** `swissarmyhammer-kanban/src/types/operation.rs` — add `Noun::Project` and `Noun::Projects` variants
- **Modify** `swissarmyhammer-kanban/src/types/mod.rs` — re-export `ProjectId`
- **Modify** `swissarmyhammer-kanban/src/error.rs` — add `ProjectNotFound`, `ProjectHasTasks` error variants
- **Modify** `swissarmyhammer-kanban/src/dispatch.rs` — add dispatch routes for all project operations
- **Modify** `swissarmyhammer-kanban/src/lib.rs` — declare and re-export `project` module

### Pattern to follow:
Use `swissarmyhammer-kanban/src/swimlane/add.rs` as the template — same entity read/write pattern, same `Entity::new("project", id)`, same JSON conversion helper.

### Key difference from swimlane:
Project has `description` and `color` fields in addition to `name` and `order`.

## Acceptance Criteria
- [ ] `ProjectId` type defined
- [ ] `Noun::Project` and `Noun::Projects` variants exist
- [ ] All 5 operations (add, get, update, delete, list) implemented with tests
- [ ] Delete fails when tasks reference the project
- [ ] Dispatch routes work for all project verb/noun combinations
- [ ] `cargo test -p swissarmyhammer-kanban` passes for project operations

## Tests
- [ ] Unit tests in each operation file (add, get, update, delete, list)
- [ ] Dispatch integration tests for project operations in `dispatch.rs`
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #swimlane-to-project