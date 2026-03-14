---
depends_on:
- 01KKP5X0G834XCJ7X9FK8TQSY4
position_column: done
position_ordinal: 9a80
title: Wire app.search command and keybindings
---
## What
Make the existing `app.search` placeholder in `app-shell.tsx` functional. This is a **frontend-only** command (same pattern as `app.command`, `app.palette`, `app.dismiss`) — it stays in the `globalCommands` array with an `execute` handler. It does NOT go in `ui.yaml` because `app.*` commands are frontend-scoped, not Rust-dispatched.

**Architecture note:** The codebase has two command namespaces:
- `app.*` — frontend-only, defined in `app-shell.tsx` globalCommands with local `execute` handlers
- `ui.*` — Rust-side, defined in `ui.yaml` with `Command` trait impls

`app.search` is purely a UI action (open palette in search mode), so it follows the `app.*` pattern.

**Files:**
- `kanban-app/ui/src/components/app-shell.tsx` — add `paletteMode` state ("command" | "search"), fill in `app.search` execute handler, pass mode to CommandPalette
- `kanban-app/ui/src/lib/keybindings.ts` — add `"/": "app.search"` to vim, `"Mod+F": "app.search"` to CUA/emacs binding tables

**Approach:**
- Update existing `app.search` placeholder's `execute` to: `setPaletteOpen(true); setPaletteMode("search"); setMode("command");`
- Update `app.command` / `app.palette` execute to also: `setPaletteMode("command")`
- Pass `paletteMode` to `CommandPalette` as `mode` prop
- Closing palette resets mode to "command"
- NavBar search button dispatches `app.search` through `useExecuteCommand()` — same dispatch path as keyboard

## Acceptance Criteria
- [ ] `app.search` has a working execute handler in globalCommands
- [ ] `/` (vim) and `Cmd+F` (CUA/emacs) trigger `app.search` through scope chain
- [ ] `:` (vim) and `Cmd+Shift+P` still open command mode
- [ ] `app.search` shows in command palette (searchable by name "Search")
- [ ] NavBar search button uses `useExecuteCommand("app.search")`

## Tests
- [ ] Existing keybinding tests pass
- [ ] Manual: all keybinding combos open correct mode