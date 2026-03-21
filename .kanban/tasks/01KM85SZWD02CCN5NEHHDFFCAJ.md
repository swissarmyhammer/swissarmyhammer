---
assignees:
- claude-code
depends_on:
- 01KM85SH3GXFQSE4MMAPGX9M9C
position_column: done
position_ordinal: ffffffffffd580
title: Add UIState read endpoint and React hook
---
## What

React needs a clean way to read UIState. Currently it reads via `get_ui_context` (Tauri cmd) and various individual getters. Replace with a single subscription model.

### Approach
- Add a `get_ui_state` Tauri command that returns the full UIState as JSON (replaces `get_ui_context`, `get_keymap_mode`, etc.)
- Emit a `ui-state-changed` Tauri event whenever UIState mutates (the `UIStateChange` enum already describes what changed)
- Create a React context `UIStateProvider` + `useUIState()` hook that:
  1. Fetches full UIState on mount via `get_ui_state`
  2. Listens for `ui-state-changed` events and patches local state
  3. Exposes typed accessors: `keymapMode`, `activeViewId`, `inspectorStack`, etc.

### Files to create/modify
- `kanban-app/src/commands.rs` — add `get_ui_state` command
- `kanban-app/ui/src/lib/ui-state-context.tsx` — new React context + hook
- `kanban-app/ui/src/App.tsx` — wrap with `UIStateProvider`

### What NOT to do
- Do NOT remove existing `get_ui_context`, `get_keymap_mode` yet — they'll be removed when their consumers migrate
- Do NOT remove existing React contexts that read this state (keymap-context, etc.) — migrate consumers in later cards

## Acceptance Criteria
- [ ] `get_ui_state` returns full UIState as JSON
- [ ] `ui-state-changed` event fires on every UIState mutation
- [ ] `useUIState()` hook provides typed access to all UIState fields
- [ ] Hook updates reactively when backend state changes

## Tests
- [ ] `kanban-app/ui/src/lib/ui-state-context.test.tsx` — test hook provides state and updates on events
- [ ] `pnpm --filter kanban-app test` passes