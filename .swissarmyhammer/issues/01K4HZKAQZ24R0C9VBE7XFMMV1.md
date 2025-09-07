# Create swissarmyhammer-todo Crate for Todo Management

## Problem

Todo management functionality is currently part of the main `swissarmyhammer` crate, preventing `swissarmyhammer-tools` from being independent. The tools crate uses:

- `swissarmyhammer::todo::{TodoStorage, TodoId}`

## Solution

Create a new `swissarmyhammer-todo` crate that contains all todo management functionality.

## Components to Extract

- `TodoStorage` trait and implementations
- `TodoId` type and related functionality
- Todo creation, completion, and querying logic
- Todo file format handling (`.todo.yaml` files)

## Files Currently Using Todo Functionality

- `swissarmyhammer-tools/src/mcp/tools/todo/create/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/todo/show/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/todo/mark_complete/mod.rs`

## Implementation Steps

1. Create new `swissarmyhammer-todo` crate in workspace
2. Move todo-related code from main crate to new crate
3. Update `swissarmyhammer-tools` to depend on `swissarmyhammer-todo`
4. Update all imports from `swissarmyhammer::todo::` to `swissarmyhammer_todo::`
5. Remove todo module from main crate

## Acceptance Criteria

- [ ] New `swissarmyhammer-todo` crate created
- [ ] All todo functionality moved and working independently
- [ ] `swissarmyhammer-tools` uses new crate directly
- [ ] All todo tests pass
- [ ] No dependency on main `swissarmyhammer` crate