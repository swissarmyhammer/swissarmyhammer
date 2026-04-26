---
assignees:
- claude-code
depends_on:
- 01KM85SZWD02CCN5NEHHDFFCAJ
position_column: done
position_ordinal: ffffffffffffa580
title: 'Migrate keymap_mode: remove Tauri cmd, route through dispatch_command'
---
## What

`keymap_mode` is duplicated in AppConfig and UIState. The `set_keymap_mode` and `get_keymap_mode` Tauri commands bypass dispatch_command. The YAML commands `settings.keymap.vim/cua/emacs` already exist but the frontend calls the direct Tauri command instead.

### Changes
- Wire `settings.keymap.*` command implementations to mutate UIState (they may already — check)
- Remove `set_keymap_mode` Tauri command
- Remove `get_keymap_mode` Tauri command
- Frontend: replace `invoke(\"set_keymap_mode\")` with `dispatch_command(\"settings.keymap.*\")`
- Frontend: replace `invoke(\"get_keymap_mode\")` with reading from `useUIState()`
- Remove `keymap_mode` from AppConfig (UIState persists it now)
- Migrate existing React `useKeymap` / keymap-context consumers to `useUIState().keymapMode`

## Acceptance Criteria
- [ ] No direct `invoke(\"set_keymap_mode\")` or `invoke(\"get_keymap_mode\")` in frontend
- [ ] Keymap changes go through `dispatch_command`
- [ ] Keymap mode persists across app restarts (via UIState persistence)
- [ ] `set_keymap_mode` and `get_keymap_mode` Tauri commands removed

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes