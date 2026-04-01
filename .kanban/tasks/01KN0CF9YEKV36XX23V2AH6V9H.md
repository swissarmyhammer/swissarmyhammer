---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9080
title: '[Info] Undo/redo architecture is clean — backend-only with frontend passthrough'
---
**Files**: `swissarmyhammer-entity/src/undo_stack.rs`, `swissarmyhammer-entity/src/undo_commands.rs`, `kanban-app/ui/src/lib/undo-context.tsx`\n\n**Observation**: The undo/redo system is well-architected:\n- `UndoStack` is a bounded, pointer-based stack persisted as YAML\n- Transaction dedup prevents multi-write commands from creating duplicate entries\n- `UndoCmd`/`RedoCmd` are entity-layer commands reusable outside kanban\n- Frontend `UndoProvider` is a pure passthrough — zero undo logic in TypeScript\n- `get_undo_state` Tauri command provides `can_undo`/`can_redo` for UI state\n- Flush gate in dispatch correctly includes `app.undo`/`app.redo` for entity emission\n\nThe test coverage in `undo_redo.rs` and `undo_redo_stack.rs` is thorough, covering round-trips, sequences, edge cases, and delete/restore cycles.\n\n**Severity**: Info (positive)\n**Layer**: Design/Architecture"