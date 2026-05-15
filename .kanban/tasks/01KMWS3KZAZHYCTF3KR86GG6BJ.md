---
assignees:
- claude-code
depends_on:
- 01KMWS2PV9EVCK6J7NXVMRJAZG
position_column: done
position_ordinal: ffffffffffffffffcc80
title: Implement entity.paste command
---
## What

Add YAML definition and Rust implementation for `entity.paste`. Paste creates a new task from clipboard data in the target column.

### Files to modify
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — add 1 YAML command entry
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — add PasteCmd
- `swissarmyhammer-kanban/src/commands/mod.rs` — register command, update count (31 → 32)

### YAML definition

```yaml
- id: entity.paste
  name: Paste
  undoable: true
  context_menu: true
  keys:
    cua: Mod+V
    vim: p
  params:
    - name: column
      from: scope_chain
      entity_type: column
```

No explicit `scope` required — availability is checked dynamically (clipboard non-empty + column or board in scope).

### Rust implementation

**PasteCmd**:
- `available()` = UIState clipboard is `Some` AND (`has_in_scope(\"column\")` OR `has_in_scope(\"board\")`)
- `execute()`:
  1. Read clipboard from UIState
  2. Determine target column: `resolve_entity_id(\"column\")`, or if only board in scope, load columns and pick the first one
  3. Determine position: if task is in scope chain (focused task), compute ordinal to place after it. Otherwise, compute ordinal for first position in column.
  4. Create new task via `AddTask` operation with fields from clipboard snapshot
  5. Return `{ pasted: new_task_id, from_clipboard: original_entity_id }`

Position logic reuses `compute_ordinal_for_neighbors` from `task_helpers` (same pattern as `MoveTaskCmd`).

Clipboard persists after paste (can paste multiple times).

## Acceptance Criteria
- [ ] YAML entry for entity.paste in entity.yaml
- [ ] PasteCmd creates a new task from clipboard data in the correct column
- [ ] Position: after focused task if one exists, otherwise first position
- [ ] Falls back to first column when only board is in scope
- [ ] Clipboard remains populated after paste
- [ ] Registered in mod.rs, command count updated

## Tests
- [ ] `swissarmyhammer-kanban/src/commands/mod.rs` — availability tests: available with clipboard + column, available with clipboard + board, not available without clipboard, not available without column/board
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` passes"
<parameter name="assignees">[]