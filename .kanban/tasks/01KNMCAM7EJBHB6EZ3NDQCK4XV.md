---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffc280
title: Fix InspectButton dispatching ui.inspect without target — inspects wrong entity on first click
---
## What

`InspectButton` in `kanban-app/ui/src/components/entity-card.tsx:154-174` dispatches `ui.inspect` with no `target` in DispatchOptions. `useDispatchCommand` (command-scope.tsx:329) prefers `FocusedScopeContext` over `CommandScopeContext`, so the scope chain is built from whichever entity was **previously focused** — not the card containing the button. The backend (`ui_commands.rs:40-44`) falls back to `first_inspectable(scope_chain)`, inspecting the wrong entity.

The first click also shifts focus (via FocusScope's onClick → `setFocus(moniker)` at focus-scope.tsx:127), so the **second** click works correctly. This is why it feels like "always takes two clicks."

**Compare**: `handleDoubleClick` in `focus-scope.tsx:212` correctly passes `dispatch({ target: moniker })` and always inspects the right entity.

### Fix

Pass the entity moniker as `target` to the dispatch call, matching the pattern already used by double-click.

### Subtasks

- [ ] Add `entityMoniker` prop to `InspectButton` in `entity-card.tsx`
- [ ] Change `dispatch()` to `dispatch({ target: entityMoniker })` at line 165
- [ ] Update the docstring on `InspectButton` (lines 147-153) to remove the stale claim that "scope chain from context already includes the entity moniker" — the scope chain comes from *focused* context, not tree context, which is the entire bug

## Acceptance Criteria

- [ ] Clicking (i) on a card inspects that card's entity on the first click, regardless of which entity was previously focused
- [ ] Double-click inspect behavior is unchanged
- [ ] Context menu inspect behavior is unchanged

## Tests

- [ ] Update `kanban-app/ui/src/components/entity-card.test.tsx` — the existing test at line 180 ("(i) button dispatches ui.inspect to the backend with entity in scope chain") should verify that the `target` field is set to the entity moniker in the dispatch_command IPC args
- [ ] Add a new test: render two EntityCards, focus Card A, click (i) on Card B — assert that `dispatch_command` is called with Card B's moniker as `target`
- [ ] Run `cd kanban-app/ui && npx vitest run entity-card --reporter=verbose` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.