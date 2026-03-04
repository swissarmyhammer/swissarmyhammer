---
position_column: done
position_ordinal: i9
title: Route update_entity_field through KanbanOperationProcessor
---
**Review finding: W2 (warning)**

`swissarmyhammer-kanban-app/src/commands.rs` — `update_entity_field`

This command manually calls `ectx.set_transaction()` / `ectx.write()` / `ectx.clear_transaction()` directly, bypassing KanbanOperationProcessor. This means:
1. No activity log entry written
2. No board auto-init
3. No ensure_directories()

## Fix approach
Create an `UpdateEntityField` operation type implementing `Execute` and route it through the processor like all other commands.

- [ ] Create UpdateEntityField operation in swissarmyhammer-kanban
- [ ] Route update_entity_field Tauri command through processor
- [ ] Verify activity log includes field updates
- [ ] Run full test suite