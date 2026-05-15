---
position_column: done
position_ordinal: ff9580
title: Register global commands and wire up AppShell with CommandScope
---
Phase 1 deliverable from app-architecture.md.

Wire everything together: wrap the app in the global CommandScope, register global commands, integrate keybindings, palette, undo stack, and mode indicator into the existing App.tsx.

## Global commands to register (from architecture doc)
- `app.command` — opens command palette (`:`)
- `app.palette` — opens command palette (`Mod+Shift+P`)
- `app.undo` — pops undo stack, dispatches undo
- `app.redo` — pops redo stack, dispatches redo
- `app.dismiss` — closes palette/panels (Escape)
- `app.save` — placeholder for now
- `app.search` — placeholder for now
- `app.keymap` — switch keymap mode
- `app.theme` — placeholder for now
- `app.help` — placeholder for now

## What to build

### AppShell wrapper
- Wraps the existing App component tree
- Provides: CommandScope (global), KeybindingProvider, UndoStackProvider, AppModeProvider
- Registers global commands in the root CommandScope

### Integration
- Replace the ad-hoc Escape handler in App.tsx with the command system (app.dismiss)
- Remove hardcoded keydown listeners — everything goes through keybindings → scope chain
- Command palette renders at the app level (portal)
- Mode indicator renders at the bottom

### Phase 1 scope chain should look like:
```
CommandScope (global)
  └─ <AppShell />     ← focused

:help works. Mod+Shift+P shows global commands. : opens same palette.
```

## Files
- `ui/src/components/app-shell.tsx` — wraps App with all providers
- Modify `ui/src/App.tsx` — remove ad-hoc key handlers, add palette + mode bar
- Modify `ui/src/main.tsx` — wrap with AppShell if needed

## Checklist
- [ ] Create AppShell component with all providers
- [ ] Register all global commands
- [ ] Wire app.command and app.palette to open the palette
- [ ] Wire app.undo and app.redo to the undo stack
- [ ] Wire app.dismiss to close palette and panels
- [ ] Remove ad-hoc Escape handler from App.tsx
- [ ] Render CommandPalette at app level
- [ ] Render ModeIndicator at bottom
- [ ] Verify : opens palette in vim mode
- [ ] Verify Mod+Shift+P opens palette in all modes
- [ ] Verify u undoes in vim mode, Mod+Z in cua mode
- [ ] Tests
- [ ] Run test suite