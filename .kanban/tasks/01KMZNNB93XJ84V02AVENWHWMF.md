---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Eliminate frontend globalCommands — move remaining commands to backend
---
## What

The frontend app-shell.tsx still has ~20 commands with client-side `execute` callbacks. These need to be moved to backend dispatch or eliminated.

### Commands to handle

**UI commands (palette, dismiss, search):**
- `app.command` — opens command palette. Backend: emit event, frontend listens.
- `app.search` — opens search palette. Backend: emit event.
- `app.dismiss` — closes palette/inspector. Backend: already has ui.palette.close + ui.inspector.close.

**Navigation commands:**
- `nav.up`, `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last` — these broadcast through EntityFocusContext. These are purely frontend focus movement. Keep as frontend keybinding handlers OR add backend nav commands.

**Settings commands:**
- `settings.keymap.vim/cua/emacs` — already have backend impls.
- `app.resetWindows` — calls `invoke('reset_windows')`. Needs backend Command impl.

**Board switching (dynamic):**
- Open boards list → each becomes a switchable command. Generated from UIState open boards.

**About:**
- `app.about` — no-op currently. Can stay as a no-op backend command.

### Approach
1. Commands with backend impls: remove frontend `execute`, dispatch goes to backend
2. Commands that emit events: add backend impls that return result markers, Tauri layer emits events
3. Nav commands: keep as frontend keybinding handlers (purely UI focus movement)
4. Dynamic board list: generate in `commands_for_scope`

## Acceptance Criteria
- [ ] Frontend globalCommands array reduced to nav commands only (or eliminated entirely)
- [ ] All dispatched commands go through backend
- [ ] Tests for new backend command impls"
<parameter name="assignees">[]