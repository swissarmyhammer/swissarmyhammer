---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff9d80
title: Migrate grid keybindings to scope-based dispatch
---
## What\n\nGridView currently has its own `window.addEventListener('keydown')` handler with a hardcoded switch statement for j/k/arrows/etc. Now that `createKeyHandler` supports scope bindings via `extractScopeBindings`, the grid should use the same pattern as the inspector:\n\n1. Define grid navigation commands with `keys` property on their `CommandDef`\n2. Remove the grid's own keydown listener\n3. Let the global `KeybindingHandler` dispatch through the focused scope\n\nThis unifies all keyboard navigation through one handler.\n\n### Files to modify\n- `kanban-app/ui/src/components/grid-view.tsx` — move key mappings to command `keys`, remove keydown listener\n\n## Acceptance Criteria\n- [ ] Grid navigation works via scope-based dispatch (no own keydown listener)\n- [ ] All existing grid keybinding tests still pass\n- [ ] Inspector and grid keybindings don't conflict"