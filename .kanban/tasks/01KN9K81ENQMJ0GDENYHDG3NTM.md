---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffb680
title: Fix multi-window perspective — scopeChain missing window label, no boardPath
---
## What

When multiple windows show the same board, only the `main` window shows the active perspective. Secondary windows show no active selection.

### Root cause

`PerspectiveProvider` uses `backendDispatch` directly with hardcoded `scopeChain: []`. The root `CommandScopeProvider` in `App.tsx:571` already provides `window:${WINDOW_LABEL}` as a moniker, so the scope chain is available via `useContext(CommandScopeContext)` + `scopeChainFromScope()`. But `PerspectiveProvider` doesn't read it — it bypasses the context entirely.

The backend's `SetActivePerspectiveCmd` falls back to `\"main\"` when no `window:` moniker is in the scope chain. So all windows write to the `main` slot.

### Fix

In `kanban-app/ui/src/lib/perspective-context.tsx`:

1. Import `useContext` for `CommandScopeContext` and `scopeChainFromScope` from `@/lib/command-scope`
2. Read the scope chain from context: `const scope = useContext(CommandScopeContext); const chain = scopeChainFromScope(scope);`
3. Use `chain` in all `backendDispatch` calls instead of `scopeChain: []`
4. Also read `boardPath` from `ActiveBoardPathContext` and pass it so `perspective.list` targets the correct board

Same fix in `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
5. The `handleAdd` and `commitRename` calls also hardcode `scopeChain: []` — read from context instead

This follows the same pattern as `useExecuteCommand` at `command-scope.tsx:285-301`.

## Acceptance Criteria
- [ ] Opening two windows on the same board, both show perspective tabs with active selection
- [ ] Clicking a tab in window B sets active perspective in window B (not main)
- [ ] Each window independently tracks its own active perspective
- [ ] `perspective.list` includes `boardPath` so it targets the correct board

## Tests
- [ ] Update `perspective-context.test.tsx` assertions to expect `scopeChain: [\"window:main\"]` (from mocked scope context)
- [ ] Update `perspective-tab-bar.test.tsx` if needed
- [ ] `pnpm test` from `kanban-app/ui/` — all pass