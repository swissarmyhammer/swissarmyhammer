---
assignees:
- claude-code
depends_on:
- 01KMTJ1M18XKCE1GBG4EJEEYAH
position_column: done
position_ordinal: ffffffffffffffc380
title: Guard global undo/redo when CM6 editor is active
---
## What

When a CodeMirror 6 editor has focus, Cmd+Z/Cmd+Shift+Z must do CM6's local undo/redo (editor buffer history), NOT our global entity undo/redo. There are two paths that need guarding:

### Path 1: Keyboard handler (`keybindings.ts`)
Currently `createKeyHandler` skips non-modifier keys inside `.cm-editor` but DOES fire modifier keys like Cmd+Z. For `app.undo` and `app.redo` specifically, we need to skip when the target is inside `.cm-editor`.

**Change in `kanban-app/ui/src/lib/keybindings.ts`:**
- After the existing editable-context check (line ~204), add a check for modifier keys that conflict with editor undo: if `normalized` matches `Mod+Z` or `Mod+Shift+Z` and `target.closest('.cm-editor')`, skip (let CM6 handle it).
- Keep this narrowly scoped — only these two shortcuts, not all modifier keys inside editors (we still want Cmd+S, Cmd+P etc to work globally).

### Path 2: Native menu event (`app-shell.tsx`)
When Edit > Undo fires via native menu, it emits `menu-command` → `executeCommand("app.undo")`. This bypasses the key handler entirely. Need a guard in `executeCommand` or in the menu-command listener.

**Change in `kanban-app/ui/src/components/app-shell.tsx`:**
- In the `menu-command` listener, for `app.undo` and `app.redo` specifically, check if `document.activeElement?.closest('.cm-editor')` — if so, skip the dispatch (the native menu accelerator already triggered CM6's undo via the DOM).

### Alternative (simpler): Guard in `executeCommand`
Instead of guarding in two places, add the CM6 check once in `executeCommand()` for these two command IDs. If focus is inside `.cm-editor`, return false without dispatching.

**Files to modify:**
- `kanban-app/ui/src/lib/keybindings.ts` — skip Mod+Z/Mod+Shift+Z inside .cm-editor
- OR `kanban-app/ui/src/components/app-shell.tsx` — guard in executeCommand

## Acceptance Criteria
- [ ] Cmd+Z in CM6 editor does editor undo (CM6 history), not global undo
- [ ] Cmd+Shift+Z in CM6 editor does editor redo, not global redo
- [ ] Cmd+Z outside CM6 editors does global undo
- [ ] Edit > Undo menu item does not trigger global undo when CM6 has focus
- [ ] Other modifier shortcuts (Cmd+P, Cmd+K) still work globally even with CM6 focused

## Tests
- [ ] Manual: type in text editor → Cmd+Z reverts text, not the entity creation
- [ ] Manual: click outside editor → Cmd+Z does global undo
- [ ] `pnpm test` passes