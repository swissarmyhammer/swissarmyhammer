---
assignees:
- claude-code
position_column: todo
position_ordinal: aa80
title: Add tag.add command for creating tags from grid view and command palette
---
## What

The tags grid dispatches `tag.add` via `grid.newBelow`, but no `tag.add` command exists — there is no `AddTagCmd` struct and no registration in the command map. Tags are currently only created implicitly via `#tag` patterns in task descriptions or through the MCP/dispatch layer.

**Files to modify:**

1. `swissarmyhammer-kanban/src/commands/mod.rs` — register `"tag.add"` in `register_commands()`, pointing to a new `AddTagCmd`

2. `swissarmyhammer-kanban/src/commands/tag_commands.rs` (new file, or add to existing entity_commands if that's where `TagUpdateCmd` lives) — implement `AddTagCmd`:
   - `available()` → always `true` (tags have no positional requirements like columns)
   - `execute()` → construct `crate::tag::AddTag` operation with:
     - `name` from `ctx.arg("name")` or `ctx.arg("title")`, defaulting to `"new-tag"`
     - Generate slug ID from name (same pattern as `AddProjectCmd` — lowercase, non-alphanum → hyphens)
   - Follow the `AddProjectCmd` pattern in `project_commands.rs` since both are simple entity types without positional requirements

3. `swissarmyhammer-kanban/builtin/entities/tag.yaml` — add `tag.add` command declaration (no scope requirement, undoable: true)

**Reference:** `AddProjectCmd` in `swissarmyhammer-kanban/src/commands/project_commands.rs` is the closest pattern to follow.

## Acceptance Criteria
- [ ] `tag.add` command exists and is registered in the command map
- [ ] Dispatching `tag.add` with `{ title: "my-tag" }` or `{ name: "my-tag" }` creates a tag entity
- [ ] Dispatching `tag.add` without args creates a tag with a default name
- [ ] Grid view `grid.newBelow` on a tags grid successfully creates a tag

## Tests
- [ ] Add test for `AddTagCmd` — creates tag with provided name
- [ ] Add test for `AddTagCmd` — creates tag with default name when no args
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.