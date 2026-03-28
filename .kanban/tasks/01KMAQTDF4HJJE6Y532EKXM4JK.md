---
assignees:
- claude-code
depends_on:
- 01KMAQSRXFHW8RMACC3P6P5SQD
position_column: done
position_ordinal: ffffffffffff9b80
title: Inspector command scope and keybindings
---
## What

Register inspector-specific commands in the command scope system, following the same pattern as `GridView` (`kanban-app/ui/src/components/grid-view.tsx` lines 186-275).

Create an `InspectorFocusBridge` component (analogous to `GridFocusBridge`) that:

1. **Wraps `EntityInspector`** in a `CommandScopeProvider` with inspector navigation commands.
2. **Registers commands** with keymap-aware bindings:
   - `inspector.moveUp` — keys: `{ vim: "k", cua: "ArrowUp" }`
   - `inspector.moveDown` — keys: `{ vim: "j", cua: "ArrowDown" }`
   - `inspector.edit` — keys: `{ vim: "i", cua: "Enter" }` → enters edit mode on focused field
   - `inspector.escape` — keys: `{ vim: "Escape", cua: "Escape" }` → exits edit mode (returns to field nav)
   - `inspector.moveToFirst` — keys: `{ vim: "g g", cua: "Home" }` (gg via sequence table)
   - `inspector.moveToLast` — keys: `{ vim: "G", cua: "End" }`
   - `inspector.nextField` — keys: `{ cua: "Tab" }` → moveDown (CUA tab-between-fields)
   - `inspector.prevField` — keys: `{ cua: "Shift+Tab" }` → moveUp
3. **Integrates with entity focus**: Registers the inspector scope under the inspected entity's moniker so the command chain resolves through inspector → app.

The `SlidePanel` or `InspectorPanel` in `App.tsx` should wrap the `EntityInspector` with this scope provider. The inspector scope should be a child of the global scope (so app.dismiss still works to close the panel).

### Files to create/modify
- Create: `kanban-app/ui/src/components/inspector-focus-bridge.tsx`
- Modify: `kanban-app/ui/src/App.tsx` — wrap `EntityInspector` inside InspectorFocusBridge/CommandScopeProvider

## Acceptance Criteria
- [ ] Inspector commands are registered in the command scope when a panel is open
- [ ] j/k (vim) or arrows (CUA) navigate between fields when inspector is focused
- [ ] Enter/i starts editing, Escape returns to field navigation
- [ ] Tab/Shift+Tab navigate fields in CUA mode
- [ ] Commands resolve through inspector scope → global scope chain

## Tests
- [ ] Test that inspector commands are registered when panel mounts
- [ ] Test that command execution calls the correct useInspectorNav methods
- [ ] Run: `cd kanban-app && npx vitest run src/components/inspector-focus-bridge.test.tsx`