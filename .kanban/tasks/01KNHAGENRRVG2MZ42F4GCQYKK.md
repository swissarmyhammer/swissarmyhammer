---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffd80
title: entity.unarchive should only be available for archived entities
---
## What

`entity.unarchive` currently shows in the context menu for all entities with a target moniker (`available` returns `ctx.target.is_some()`). It should only be available when the entity is actually in the archive.

### Problem

`Command::available()` is synchronous — it can't do async filesystem checks to see if an entity lives in `.archive/`. The right approach is to let the caller mark archived entities with a moniker convention so `available()` can check synchronously.

### Approach: archived moniker tag

Add an `archive:` prefix or suffix convention to monikers in the scope chain when the entity is from the archive view. For example, an archived task would have moniker `task:01ABC:archive` instead of `task:01ABC`. This is set by the frontend when rendering the archive list view.

**Alternative considered**: Store archived entity IDs in `UIState` — rejected because it couples transient UI state to command availability and doesn't scale.

### Files to modify

1. **`swissarmyhammer-kanban/src/commands/entity_commands.rs`** (`UnarchiveEntityCmd::available`)
   - Change from `ctx.target.is_some()` to check for `:archived` suffix on the target moniker: `ctx.target.as_deref().is_some_and(|t| t.ends_with(":archived"))`
   - In `execute()`, strip `:archived` suffix before parsing the moniker

2. **`swissarmyhammer-kanban/src/commands/entity_commands.rs`** (`ArchiveEntityCmd::available` if it exists, or the `entity.archive` command in scope_commands)
   - Ensure `entity.archive` is NOT available when the target has `:archived` suffix (can't archive what's already archived)

3. **`kanban-app/ui/src/components/data-table.tsx`** or the archive list view component
   - When rendering archived entities, construct the scope chain moniker with `:archived` suffix (e.g. `task:01ABC:archived`)

4. **`swissarmyhammer-kanban/src/scope_commands.rs`** (`check_available`)
   - The `target` passed to `check_available` may need to carry the `:archived` tag through, or strip it before passing to non-archive-aware commands

### Design note

The `:archived` suffix is a scope chain convention, not a persistence change. It's set by the view that renders archived entities and consumed only by `available()` checks. The `execute()` path strips it before doing real work. This keeps the approach purely in the command availability layer with no filesystem or async dependencies.

## Acceptance Criteria

- [ ] `entity.unarchive` context menu item does NOT appear when right-clicking a live (non-archived) entity
- [ ] `entity.unarchive` context menu item DOES appear when right-clicking an entity in the archive list view
- [ ] `entity.archive` does NOT appear when right-clicking an entity in the archive list view
- [ ] Executing `entity.unarchive` from the archive view works correctly (entity is restored)
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Tests

- [ ] `swissarmyhammer-kanban/src/commands/entity_commands.rs` — `unarchive_entity_available_when_target_has_archived_suffix` (target `task:01ABC:archived` → true)
- [ ] `swissarmyhammer-kanban/src/commands/entity_commands.rs` — `unarchive_entity_not_available_for_live_entity` (target `task:01ABC` → false)
- [ ] `swissarmyhammer-kanban/src/commands/entity_commands.rs` — `unarchive_entity_executes_with_archived_suffix` (strips suffix, unarchives correctly)
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — `unarchive_not_in_context_menu_for_live_task` (scope chain test)
- [ ] Run `cargo test -p swissarmyhammer-kanban` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.