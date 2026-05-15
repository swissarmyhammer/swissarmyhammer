---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9880
title: Defer scope-chain construction in useContextMenu; return a stable handler across renders
---
## What

`useContextMenu` in `kanban-app/ui/src/lib/context-menu.ts` builds the scope chain and wraps it in a `useCallback` with the chain as a dep on every render. Because `scopeChainFromScope(scope)` returns a fresh array each call, the `useCallback` cache always misses and the returned handler is a new function on every render.

The hook is called from three high-multiplier sites (all three confirmed by `Grep useContextMenu` in `kanban-app/ui/src`):

- `kanban-app/ui/src/components/focus-scope.tsx::FocusScopeInner` — one per cell with `renderContainer=true` (every `GridCellScope`).
- `kanban-app/ui/src/components/data-table.tsx::EntityRow` — one per row.
- `kanban-app/ui/src/components/grid-view.tsx::GridBody` — one per view body.

On a 2000-row grid with ~6 field columns that's ~14,001 invocations per grid render, each allocating an 8-9 element scope-chain array AND a fresh closure. The closures also defeat React's ability to skip children via prop-identity comparisons, compounding the re-render cost. And because `scopeChainFromScope` walks through `FocusedScopeContext` (via `scope.parent`), the handler churns on every focus change as well.

The backend IPCs (`list_commands_for_scope`, `show_context_menu`) are already correctly deferred — they only fire when the user right-clicks. What is NOT deferred is the scope-chain walk + closure allocation that prepares the handler, and that work is what runs 14k times per render.

**Approach** — hoist the scope to a ref and walk it inside the handler:

```ts
export function useContextMenu(): (e: React.MouseEvent) => void {
  const scope = useContext(CommandScopeContext);
  const scopeRef = useRef(scope);
  scopeRef.current = scope;                      // updated each commit
  return useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const scopeChain = scopeChainFromScope(scopeRef.current);
    invoke<ResolvedCommand[]>("list_commands_for_scope", {
      scopeChain,
      contextMenu: true,
    })
      .then((commands) => {
        // ... existing item construction, uses `scopeChain` local var
      })
      .catch(console.error);
  }, []);
}
```

Properties after the change:
- `useCallback([])` keeps the returned handler's identity stable across renders.
- `scopeRef.current` is React-updated in commit, so right-click always reads the scope that was committed at the previous render — correct for the user's actual click moment.
- `scopeChainFromScope` runs exactly once per right-click, not 14k times per grid render.

**Files**
- `kanban-app/ui/src/lib/context-menu.ts` — primary change (the hook body).
- `kanban-app/ui/src/lib/context-menu.test.tsx` — regression test.

Call sites (`focus-scope.tsx`, `data-table.tsx`, `grid-view.tsx`) need no changes — the hook's external contract is unchanged.

### Subtasks
- [x] Refactor `useContextMenu` to use `scopeRef` + empty deps, moving `scopeChainFromScope` into the handler body.
- [x] Add regression test: render a consumer that captures the handler reference across two re-renders, assert the ref is identical (strict `===`), and assert `scopeChainFromScope` (or the first `invoke` arg) reflects a late-bound scope change between the renders.
- [x] Verify existing `context-menu.test.tsx` tests still pass (behavior when right-clicking, items fanned to `show_context_menu`, separators).

## Acceptance Criteria
- [x] Returned handler from `useContextMenu()` has identical reference across consecutive renders when nothing else about the hook's inputs changed.
- [x] Scope chain passed to `list_commands_for_scope` reflects the scope **at right-click time**, not the scope from when the handler was first created — regression test proves this.
- [x] No behavioral change in the native context menu: same items, same separators, same order, same `cmd`/`target`/`scope_chain` payload shipped to `show_context_menu`.
- [x] Existing tests in `kanban-app/ui/src/lib/context-menu.test.tsx` pass unchanged.
- [ ] Manual smoke: right-click on a grid row, a column header (where grouping toggles stopPropagation), and grid whitespace — each surfaces the correct command set with no regressions.

## Tests
- [x] Add test to `kanban-app/ui/src/lib/context-menu.test.tsx` — `"returned handler is reference-stable across renders"`. Setup: render a consumer inside a `CommandScopeProvider`; capture the handler via `renderHook` `result.current`; `rerender()`; assert `result.current === previousRef`.
- [x] Add test `"handler reflects the scope at click time, not at render time"`. Render under a scope with moniker A, capture the handler; rerender under a scope with moniker B; invoke the captured handler; assert the `scopeChain` arg to `invoke("list_commands_for_scope", ...)` starts with moniker B, not A.
- [x] Test command: `cd kanban-app/ui && npm test -- context-menu`. Expected: green.
- [x] Full suite: `cd kanban-app/ui && npm test`. Expected: green. (1340 tests / 123 files, all green.)
- [ ] Manual smoke described above.

## Workflow
- Use `/tdd` — write the two regression tests first (handler-stability + scope-at-click-time), confirm they fail against the current implementation, then refactor the hook to make them pass. #performance #frontend #commands