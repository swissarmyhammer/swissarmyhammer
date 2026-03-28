---
assignees:
- claude-code
depends_on:
- 01KM85TNG65NWR6Z17B4443FC4
position_column: done
position_ordinal: ffffffffffd880
title: 'Migrate focus scope_chain: deduplicate from AppState into UIState only'
---
## What

`scope_chain` is duplicated: `AppState.focus_scope_chain` and `UIState.scope_chain`. The `set_focus` Tauri command writes to AppState directly. UIState already has `set_scope_chain()`.

### Changes
- Route `set_focus` through UIState instead of AppState.focus_scope_chain
- Remove `AppState.focus_scope_chain` field
- All reads of scope_chain go through UIState
- `set_focus` can remain as a Tauri command (it's input routing, not data mutation) but should write to UIState

## Acceptance Criteria
- [ ] `AppState.focus_scope_chain` field removed
- [ ] `set_focus` writes to UIState.scope_chain
- [ ] `dispatch_command` reads scope_chain from UIState

## Tests
- [ ] `cargo nextest run -p kanban-app` passes