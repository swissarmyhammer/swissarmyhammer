---
position_column: done
position_ordinal: ffffb180
title: 'End-to-end integration tests: command dispatch → state → events → UI'
---
Final integration test suite that validates the full architecture works end-to-end. Primarily Rust tests that simulate full user sessions without a UI.

## Scope

- Build `TestEngine` helper: sets up board, entity cache, commands registry, UIState — everything needed to simulate the app without Tauri/React
- Write session-level integration tests that exercise the full flow:

### Command resolution tests
- Dispatch `task.untag` with scope chain `[tag:X, task:Y, board:board]` → tag removed from task
- Dispatch `task.add` with scope chain `[column:todo, board:board]` → task created in todo column with default title
- Dispatch `task.move` with scope chain `[task:X, column:src, board:board]`, target `column:dest`, args `{ drop_index: 2 }` → task moved, ordinal computed correctly
- Dispatch `entity.update_field` with scope chain and args → field updated

### Undo/redo tests
- Create task → undo → task deleted → redo → task restored
- Move task → edit title → undo → title reverted (task still in new column) → undo → task back in original column
- Undoable commands generate operation_id, non-undoable don't
- `app.undo` when nothing to undo → error or no-op

### UI state tests
- Dispatch `ui.inspect` with target `task:X` → inspector stack is `[task:X]`
- Dispatch `ui.inspect` with target `tag:Y` → stack is `[task:X, tag:Y]`
- Dispatch `ui.inspect` with target `task:Z` → stack is `[task:Z]` (primary replaces)
- Dispatch `ui.inspector.close` → stack pops
- UI commands are NOT undoable — undo skips them

### Availability tests
- `task.untag` not available without tag in scope
- `task.add` not available without column in scope
- `app.undo` not available with empty undo stack
- `app.quit` always available

### Event tests
- Command execution emits correct entity-changed events
- Events contain correct version numbers
- No events emitted when entity content unchanged

### Concurrent access simulation
- Write entity via command → externally modify file → refresh_from_disk → event emitted → cache updated

## Testing

This card IS the tests. Success criteria:
- All tests pass
- Tests cover the full dispatch → resolve → execute → emit cycle
- Tests run without Tauri, without React, without a window
- Tests complete in under 5 seconds total