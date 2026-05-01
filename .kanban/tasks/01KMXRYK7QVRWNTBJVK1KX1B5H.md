---
assignees:
- claude-code
depends_on:
- 01KMXRY4SHGCHEGMG2M8EMDA6P
position_column: done
position_ordinal: ffffffffffffffffdc80
title: Tauri endpoint + wire context menu and palette to backend
---
## What

Add Tauri command that wraps commands_for_scope. Rewrite frontend context menu and command palette to be thin renderers — they call the backend with scope chain, get commands back, render them.

### Files to modify
- `kanban-app/src/commands.rs` — new `list_commands_for_scope` Tauri command wrapping `commands_for_scope`
- `kanban-app/src/main.rs` — register new Tauri command
- `kanban-app/ui/src/lib/context-menu.ts` — call `list_commands_for_scope`, render results, dispatch on click
- `kanban-app/ui/src/components/command-palette.tsx` — call `list_commands_for_scope` on open, render results
- `kanban-app/ui/src/lib/entity-commands.ts` — remove command building logic
- `kanban-app/ui/src/lib/command-scope.tsx` — remove `collectAvailableCommands` (backend does this now)

### Frontend becomes
- Context menu: `invoke('list_commands_for_scope', { scopeChain, contextMenu: true })` → render → `invoke('dispatch_command', { cmd, target })`
- Palette: `invoke('list_commands_for_scope', { scopeChain })` → fuzzy filter → render → dispatch
- No command logic, no availability checks, no name resolution

### Remove from frontend
- `useEntityCommands()` command building (keep schema loading for field display)
- `buildEntityCommandDefs()` 
- `resolveCommandName()` for commands (backend does this)
- `collectAvailableCommands()` — backend replacement
- `useAvailableCommands()` — backend replacement

## Acceptance Criteria
- [ ] Context menu shows correct commands from backend
- [ ] Palette shows correct commands from backend
- [ ] Frontend has zero command logic — just render and dispatch
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] Manual: right-click tag → Copy Tag, Cut Tag, Inspect Tag
- [ ] Manual: right-click task → Copy Task, Cut Task, Paste Tag (if applicable), Inspect Task, Archive Task
- [ ] Manual: palette shows same resolved names"
<parameter name="assignees">[]