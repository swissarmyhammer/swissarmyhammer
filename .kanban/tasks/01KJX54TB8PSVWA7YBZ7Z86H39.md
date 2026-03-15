---
position_column: done
position_ordinal: ffa780
title: Unify native menu and command palette — commands as single source of truth
---
The native macOS menu (built in Rust via menu.rs) and the command palette (React) are completely separate systems with duplicated functionality. Keymap switching exists in both. Undo/Redo exists in both. Quit only exists in the native menu.

## Design

Commands are the single source of truth. Both the native menu and the command palette derive from the same command registry.

### How it works

1. The command registry lives in React (CommandScope). It already has all the global commands with IDs, names, keybindings.

2. For the native menu: on app startup and when the command set changes, generate the native menu FROM the command registry. This means:
   - Emit commands to the Rust side via a Tauri event or command
   - Rust builds the native menu from the command list
   - Menu item clicks emit back to the frontend as command executions

3. For commands that are OS-level (Quit, Hide, Show All, About): these stay as PredefinedMenuItems but are ALSO registered as commands so they show in the palette.

### Commands to add
- `app.quit` — Quit the application. In palette, executes `window.close()` or Tauri quit API
- `app.keymap.vim` — Switch to vim mode
- `app.keymap.cua` — Switch to CUA mode  
- `app.keymap.emacs` — Switch to emacs mode
- `app.about` — Show about dialog
- `file.new` — New board
- `file.open` — Open board

### Immediate quick fix (before full unification)
- Add app.quit, app.keymap.vim/cua/emacs to the global commands in AppShell
- These just call the existing Tauri commands (set_keymap_mode, etc.)
- The native menu continues to work as-is
- Full menu generation from commands is a future card

## Files
- `ui/src/components/app-shell.tsx` — add missing global commands
- Future: `src/menu.rs` refactor to generate from command list

## Checklist
- [ ] Add app.quit command (calls Tauri quit API)
- [ ] Add app.keymap.vim, app.keymap.cua, app.keymap.emacs commands
- [ ] Add file.new, file.open commands (or placeholders)
- [ ] Add app.about command
- [ ] All show in command palette with correct keybindings
- [ ] Native menu continues to work (no regression)
- [ ] Tests
- [ ] Future card: generate native menu from command registry