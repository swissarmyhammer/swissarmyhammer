---
position_column: done
position_ordinal: ed80
title: Keybinding layer — key sequences to command IDs with scope resolution
---
Phase 1 deliverable from app-architecture.md.

Keybindings map key sequences to command IDs. The binding table is global per keymap mode. Resolution of the command ID goes through the scope chain.

## What to build

### Binding table
- One table per keymap mode (vim, cua, emacs)
- Maps key sequence string → command ID
- Mod = Cmd on Mac, Ctrl on Windows/Linux
- Multi-key sequences (gg, dd, zo) with ~500ms timeout pending-key buffer

### Key handler
- Global keydown listener
- Looks up key sequence in binding table for current keymap mode
- Gets command ID
- Resolves command ID through scope chain (from CommandScope)
- If resolved and available → execute
- If not resolved → unhandled (let browser handle it)

### Pending key buffer for vim
- When first key of a multi-key sequence is pressed, start buffer
- ~500ms timeout — if second key doesn't arrive, flush as single key
- Lookup table of 1-2 key sequences, not a full vim parser

### Initial bindings (from architecture doc)
```
vim:
  ":": app.command
  Mod+Shift+P: app.palette
  u: app.undo
  Mod+R: app.redo
  Escape: app.dismiss

cua:
  Mod+Shift+P: app.palette
  Mod+Z: app.undo
  Mod+Shift+Z: app.redo
  Escape: app.dismiss

emacs:
  Mod+Shift+P: app.palette
  C-/: app.undo
  C-?: app.redo
  Escape: app.dismiss
```

## Files
- `ui/src/lib/keybindings.ts` — binding tables, key handler, pending buffer
- `ui/src/lib/keybindings.test.ts` — tests

## Checklist
- [ ] Binding table data structure per keymap mode
- [ ] Key handler (global keydown → lookup → scope resolve → execute)
- [ ] Mod key resolution (Cmd on Mac, Ctrl on others)
- [ ] Multi-key pending buffer with timeout for vim
- [ ] Integration with CommandScope (resolve command ID through scope chain)
- [ ] Don't capture keys when CM6 editor has focus (CM6 handles its own keys)
- [ ] Initial vim/cua/emacs binding tables from architecture doc
- [ ] Tests
- [ ] Run test suite