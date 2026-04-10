---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
title: Perspective rename from command palette should focus active tab and enter inline edit mode
---
## What

Running "rename perspective" from the command palette (with no args) does nothing
useful today — the backend `perspective.rename` command requires `id` and
`new_name` args. The user expects it to focus the active perspective tab's name
field and enter inline edit mode (the same CM6 rename editor triggered by
double-click).

### Current state

- Double-click on a tab → calls `startRename(p.id)` → sets `renamingId` state
  → `TabButton` shows `InlineRenameEditor` instead of the name label
- Command palette → dispatches `perspective.rename` → backend errors (missing
  `new_name` arg) or does nothing because the UI has no way to enter inline
  edit mode from a command

### Fix

Register a UI-side command (`ui.perspective.startRename`) in the
`PerspectiveTabBar` component that calls `startRename(activePerspective.id)`.
This command should:

1. Be registered as a local command in a `CommandScopeProvider` wrapping the
   tab bar (or via `useMemo` commands array passed to the existing scope)
2. Require an active perspective in scope (available via `usePerspectives`)
3. Set `renamingId` to the active perspective's ID, which triggers the
   `InlineRenameEditor` in the active tab

The command also needs to be registered in the Rust command registry so it
appears in the command palette. Pattern: follow `ui.perspective.set` which
is already a UI command dispatched from the frontend.

### Files to modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — in
  `PerspectiveTabBar`, register a local `ui.perspective.startRename` command
  in a `CommandScopeProvider` (or add to the existing scope) that calls
  `startRename(activePerspective.id)` when executed
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — add
  `StartRenamePerspectiveCmd` (no-op on backend, just needs to exist in
  registry for command palette discovery)
- `swissarmyhammer-kanban/src/commands/mod.rs` — register the new command

## Acceptance Criteria

- [ ] "Rename Perspective" appears in the command palette
- [ ] Selecting it (with no args) focuses the active perspective tab and shows
      the inline CM6 rename editor
- [ ] Typing a new name and pressing Enter dispatches `perspective.rename`
      with the new name (existing flow)
- [ ] Pressing Escape cancels the rename (existing flow)
- [ ] Double-click rename continues to work as before

## Tests

- [ ] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — add test:
      dispatching `ui.perspective.startRename` sets the active perspective tab
      into rename mode
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/perspective-tab-bar.test.tsx`

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.