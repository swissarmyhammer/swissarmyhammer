---
assignees:
- claude-code
depends_on:
- 01KMTJ02YMZ071QJ6VWYTZ0X4C
position_column: done
position_ordinal: ffffffffffffffff8f80
title: Wrap undoable commands in transactions + set EntityContext extension
---
## What

Currently `dispatch_command_internal` only sets `KanbanContext` as an extension. This couples undo/redo commands to the kanban crate. Since undo/redo is a property of the entity layer, `EntityContext` should be set as its own extension so commands can reach it directly.

**Prerequisite change: Wrap EntityContext in Arc**

Currently `KanbanContext::entity_context()` returns `&EntityContext` (via `OnceCell<EntityContext>`). The extension system requires `Arc<T>`. Change:
- `swissarmyhammer-kanban/src/context.rs` — change `entities: OnceCell<EntityContext>` to `entities: OnceCell<Arc<EntityContext>>`
- `entity_context()` return type: `Result<Arc<EntityContext>>` (returns clone of the Arc)
- Update all callers in kanban crate that use `entity_context()` — they'll get `Arc<EntityContext>` instead of `&EntityContext`

**Changes in `kanban-app/src/commands.rs` → `dispatch_command_internal()`:**

1. **Set EntityContext as a direct extension:**
   - After setting the KanbanContext extension, get the EntityContext via `kanban_ctx.entity_context().await?` and call `ctx.set_extension(entity_ctx)`
   - This is the key decoupling: undo/redo commands only need EntityContext, not KanbanContext

2. **Transaction wrapping for undoable commands:**
   - Before executing an undoable command, get the EntityContext and call `generate_transaction_id()` + `set_transaction(tx_id)`
   - After execution (success or failure), call `clear_transaction()`
   - This ensures every undoable command gets a single transaction ID

**Files to modify:**
- `swissarmyhammer-kanban/src/context.rs` — wrap EntityContext in Arc, update return type
- `kanban-app/src/commands.rs` — `dispatch_command_internal()`, set extension + transaction wrapping
- All callers of `entity_context()` in kanban crate — update for Arc<EntityContext>

## Acceptance Criteria
- [ ] EntityContext stored as `Arc<EntityContext>` in KanbanContext
- [ ] EntityContext set as direct extension on CommandContext
- [ ] Undoable commands wrapped in set_transaction/clear_transaction
- [ ] Transaction cleared even on command failure
- [ ] Non-undoable commands not wrapped
- [ ] Existing tests pass

## Tests
- [ ] Existing dispatch tests still pass
- [ ] `cargo nextest run` passes across all crates