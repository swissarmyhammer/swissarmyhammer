---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff180
title: 'Inspector opens on wrong window: keyboard dispatch missing scopeChain'
---
## What

When pressing `i` to inspect a card in a secondary window, the inspector opens on a different window (often the main window). The root cause is that `dispatchCommand()` in `command-scope.tsx:244` does not pass the `scopeChain` to the backend. The backend falls back to `UIState.scope_chain` which holds the LAST focused scope chain across ALL windows — not the current window's chain.

### The scope chain flow

1. **Focus change** → `entity-focus-context.tsx:81` sends `scope_chain` including `window:X` to backend via `dispatch_command` with `cmd: 'focus.changed'`
2. Backend stores this in `UIState.scope_chain` (GLOBAL, not per-window)
3. **Keyboard command** → `dispatchCommand()` at `command-scope.tsx:244` sends `cmd`, `target`, `args`, `boardPath` — but NO `scopeChain`
4. Backend uses `UIState.scope_chain` to find window label via `window_label_from_scope()`
5. If window B was last focused, then pressing `i` in window A uses window B's scope chain → inspector opens in wrong window

### Fix

`dispatchCommand()` should include the current scope chain when dispatching to the backend. The `CommandDef` already has enough context — it comes from `useCommandResolver()` which walks the scope chain. The scope chain should be passed through to `backendDispatch()`.

Alternatively, the FocusScope that builds the scope chain (in `focus-scope.tsx:162`) could be passed alongside the command.

### Files to modify

- `kanban-app/ui/src/lib/command-scope.tsx:244` — add `scopeChain` to `backendDispatch()` call
- May need to thread the scope chain through `CommandDef` or `dispatchCommand()`

## Acceptance Criteria

- [ ] Pressing `i` on a card in window B opens inspector in window B, not window A
- [ ] Context menu inspect still works correctly per-window
- [ ] Command palette commands are scoped to the window they're invoked from

## Tests

- [ ] `command-scope.test.tsx`: dispatch via keyboard includes scopeChain with correct `window:` moniker
- [ ] `entity-focus-context.test.tsx`: verify scope chain sent to backend includes window label
- [ ] Run: `cd kanban-app/ui && npx vitest run --project unit` — all tests pass