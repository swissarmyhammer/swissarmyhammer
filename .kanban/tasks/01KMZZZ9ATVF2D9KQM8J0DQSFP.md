---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe880
title: 'Centralize windowLabel: window moniker at root of scope chain'
---
## What

The command palette (and other commands) fire in both windows because `windowLabel` is passed ad-hoc — some call sites include it, some don't. The window should be a moniker at the root of the scope chain, just like `column:todo` or `task:abc`. Every dispatch inherits it automatically.

### Approach
1. **Add root `CommandScopeProvider`** in `App.tsx` with `moniker="window:{WINDOW_LABEL}"` and `commands={[]}` wrapping the outermost provider tree.
2. **Backend: extract window from scope chain** in `dispatch_command_internal` — scan for `window:*` moniker. Falls back to explicit `window_label` parameter.
3. **Create `backendDispatch()` helper** in `command-scope.tsx` — wraps `invoke("dispatch_command", ...)`, always injects `windowLabel`.
4. **Replace all raw `invoke("dispatch_command", ...)` calls** with `backendDispatch()`.
5. **Backend safety net**: log `tracing::warn!` when neither scope chain nor `window_label` provides a window identity.

## Acceptance Criteria
- [x] Root `CommandScopeProvider` with `moniker="window:{WINDOW_LABEL}"` wraps the provider tree in App.tsx
- [x] Scope chains always include `window:*` as the root moniker (last element)
- [x] Backend `dispatch_command_internal` extracts `window:*` from scope chain and uses it as window_label
- [x] Zero raw `invoke("dispatch_command", ...)` calls remain in frontend — all go through `backendDispatch()`
- [x] Command palette opens in the correct window only (not both)

## Tests
- [x] `command-scope.test.tsx` — test that `backendDispatch()` always includes `windowLabel`
- [x] `entity-focus-context.test.tsx` — test that scope chain built from a focused entity includes `window:{label}` at the root
- [x] `cargo nextest run -p swissarmyhammer-commands` — backend test: scope chain with `window:secondary-1` resolves correct window_label
- [x] `cd kanban-app/ui && npx vitest run` — all frontend tests pass (no regressions vs baseline)
- [x] `cargo nextest run` — all 6948 backend tests pass