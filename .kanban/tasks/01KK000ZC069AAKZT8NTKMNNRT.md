---
position_column: done
position_ordinal: ffff9380
title: 'UIState in Rust: inspector stack, active view, palette, keymap'
---
Move all UI state from React contexts into a Rust-owned `UIState` struct that emits events on change.

## Scope

- Define `UIState` struct: `inspector_stack: Vec<String>` (monikers), `active_view_id: String`, `palette_open: bool`, `keymap_mode: String`
- Implement mutation methods that emit Tauri events:
  - `inspect(moniker)` — push onto stack with primary/secondary logic (task/column/board replaces stack, tag pushes)
  - `inspector_close()` — pop top
  - `inspector_close_all()` — clear stack
  - `set_active_view(id)` 
  - `set_palette_open(bool)`
  - `set_keymap_mode(mode)`
- Each mutation emits a targeted event with the full current value:
  - `"inspector-stack"` → `Vec<String>`
  - `"active-view"` → `String`
  - `"palette-open"` → `bool`
  - `"keymap-mode"` → `String`
- Store `UIState` in Tauri `AppState`
- Wire up `set_focus` Tauri command that receives scope chain from React and stores it

## Testing

- Test: `inspect("task:01XYZ")` sets stack to `["task:01XYZ"]`
- Test: `inspect("tag:01TAG")` after task pushes onto stack → `["task:01XYZ", "tag:01TAG"]`
- Test: `inspect("task:01ABC")` replaces stack (primary entity) → `["task:01ABC"]`
- Test: `inspector_close` pops top entry
- Test: `inspector_close_all` clears stack
- Test: `set_focus` stores scope chain, retrievable for command dispatch
- Test: each mutation returns the event payload that would be emitted