---
position_column: done
position_ordinal: ff8a80
title: Command palette UI — portal + CM6 input + fuzzy search
---
Phase 1 deliverable from app-architecture.md.

Single command palette popover for both `:` and `Mod+Shift+P`. Uses CM6 single-line as the filter input (not a plain input — vim motions work inside).

## What to build

### Palette component
- Plain portal + backdrop (not Radix Popover — avoids focus trap conflicts with CM6)
- Centered on viewport
- CM6 single-line input for filter text, with user's keymap mode
- Fuzzy search filters the command list as you type
- Shows keybinding hints for current keymap mode
- Grouped by scope depth (global, view, grid)
- Escape dismisses
- Enter on selected command executes it
- Arrow keys / j/k navigate the list

### CM6 integration
- Single-line CM6 instance with the user's selected keymap (vim/cua/emacs)
- Escape handling: in vim mode, first Escape goes insert→normal, second Escape dismisses palette
- onChange fires fuzzy filter

### Command data
- Uses useAvailableCommands() from CommandScope
- Each command shows: name, keybinding hint for current mode
- Fuzzy match on command name and description

### Two entry points (same UI)
- `:` opens palette (app.command) — CM6 input pre-focused
- `Mod+Shift+P` opens palette (app.palette) — same UI, same list

## Files
- `ui/src/components/command-palette.tsx` — the palette component
- `ui/src/lib/fuzzy-filter.ts` — fuzzy matching utility
- Tests

## Checklist
- [ ] Portal-rendered centered popover with backdrop
- [ ] CM6 single-line input with keymap mode
- [ ] Fuzzy search filtering
- [ ] Command list grouped by scope depth
- [ ] Keybinding hints per command
- [ ] Arrow key / j/k navigation
- [ ] Enter executes, Escape dismisses
- [ ] Vim Escape: insert→normal first, then dismiss
- [ ] Wire up app.command and app.palette commands
- [ ] Tests
- [ ] Run test suite