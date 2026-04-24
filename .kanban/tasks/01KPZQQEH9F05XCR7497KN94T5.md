---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
- 01KPZPY5F5HPXDKKHGKDEW6FNZ
- 01KPZQDAC6P0AHTQ5F08A170H4
position_column: todo
position_ordinal: ff9080
title: 'Stabilize useDispatchCommand: read scope/boardPath/setInflight from a ref so dispatch identity survives focus changes'
---
## What

`useDispatchCommand` in `kanban-app/ui/src/lib/command-scope.tsx` currently memoizes its returned dispatch callback on `[presetCmd, effectiveScope, boardPath, setInflightCount]`. `effectiveScope = focusedScope ?? treeScope`, and `focusedScope` comes from `FocusedScopeContext` — which updates on every entity focus change. So **every focus change** (arrow-key nav, cell click) produces a new `dispatch` reference at every one of the ~30 non-test call sites of this hook.

That identity churn is the root cause of several per-nav performance bugs we've already filed locally:
- **01KPZPY5F5HPXDKKHGKDEW6FNZ** — `PerspectiveProvider.refresh` re-fires `perspective.list` on every focus change because its `useCallback` depends on `dispatch`.
- Any `useEffect` / `useCallback` / `useMemo` anywhere in the app whose deps include `dispatch` re-runs on every focus change.

Fixing each consumer locally (refs, stable callbacks) works, but is a cat-and-mouse game against the hook's contract. The better fix is to make `dispatch` identity-stable at the source, so the whole consumer tree stops churning for free.

**Approach** — snapshot the context reads in a ref and access them at dispatch time, not at render-memoization time:

```ts
export function useDispatchCommand(presetCmd?: string) {
  const treeScope = useContext(CommandScopeContext);
  const focusedScope = useContext(FocusedScopeContext);
  const boardPath = useContext(ActiveBoardPathContext);
  const setInflightCount = useContext(CommandBusySetterContext);

  const latestRef = useRef({ treeScope, focusedScope, boardPath, setInflightCount });
  latestRef.current = { treeScope, focusedScope, boardPath, setInflightCount };

  return useCallback(
    async (cmdOrOpts, maybeOpts) => {
      const { treeScope, focusedScope, boardPath, setInflightCount } = latestRef.current;
      const effectiveScope = focusedScope ?? treeScope;
      const { cmdId, opts } = resolveDispatchArgs(presetCmd, cmdOrOpts, maybeOpts);
      const resolved = resolveCommand(effectiveScope, cmdId);
      if (resolved?.execute) return runFrontendExecute(cmdId, opts, resolved);
      const chain = opts.scopeChain ?? scopeChainFromScope(effectiveScope);
      return runBackendDispatch(cmdId, opts, chain, boardPath, setInflightCount);
    },
    [presetCmd],  // the only input that genuinely changes the callable's behavior
  );
}
```

Properties after the change:
- `dispatch` identity is stable across re-renders for a given `presetCmd` (same reasoning as the React docs' "effect event" pattern — this is an event handler, not a value).
- Every call reads the LATEST scope/boardPath/setter from `latestRef.current` — which React updates on every commit. The semantic is "dispatch uses whatever scope exists at click time," not "whatever existed when the handler was first created." That's the correct behavior for a user-action dispatch: if focus moves between render and click, the click should act on current focus.
- `setInflightCount` is a React state setter (stable by React guarantee) but goes through the ref anyway, so the contract is uniform and robust to future wiring changes.

**Why this is safe** — all 30 non-test call sites pass `dispatch` either directly to event handlers (click, keydown, etc.) or await its result. None depend on identity change as a signal. Stabilizing identity is a pure improvement; where a caller currently has a local `dispatchRef` workaround (e.g. `AppShell::KeybindingHandler`, the perspective-context changes in 01KPZPY5F5HPXDKKHGKDEW6FNZ), those become redundant but harmless — leave them in place for this task's scope to stay tight.

**Related follow-up after this lands** — re-evaluate whether 01KPZPY5F5HPXDKKHGKDEW6FNZ's local `dispatchRef` refactor is still needed. This task's change fixes the root cause; the local fix becomes defensive belt-and-suspenders. Not a blocker here.

**Files**
- `kanban-app/ui/src/lib/command-scope.tsx` — the hook refactor.
- `kanban-app/ui/src/lib/command-scope.test.tsx` — regression tests.

The 30 call sites require no changes; the hook's external contract (a callable returning Promise<unknown>) is unchanged.

### Subtasks
- [ ] Refactor `useDispatchCommand` to snapshot context reads into `latestRef` and keep only `[presetCmd]` as the `useCallback` dep.
- [ ] Add regression test `"dispatch identity is stable across renders when presetCmd is unchanged"`.
- [ ] Add regression test `"dispatch reads the latest focused scope at call time, not at render time"` — render under scope A, capture dispatch, rerender under scope B, invoke dispatch, assert the `scopeChain` passed to `invoke("dispatch_command", ...)` reflects scope B.
- [ ] Add regression test `"dispatch respects presetCmd identity changes"` — passing a different `presetCmd` DOES return a new callable (so `useDispatchCommand("ui.inspect")` and `useDispatchCommand("nav.up")` remain distinguishable).
- [ ] Confirm existing `command-scope.test.tsx` tests still pass.
- [ ] Run full UI test suite; investigate any regressions (should be none given the contract is unchanged).

## Acceptance Criteria
- [ ] `useDispatchCommand()` (no preset) returns a reference-stable callable across re-renders triggered by `FocusedScopeContext` changes.
- [ ] `useDispatchCommand("some.cmd")` returns a reference-stable callable across re-renders when the preset doesn't change.
- [ ] Invoking the captured dispatch after the focused scope has moved produces a backend call whose `scopeChain` argument reflects the CURRENT focused scope, not the stale one. (Regression test enforces this.)
- [ ] The existing behavior of every call site is preserved — no user-visible change to dispatch semantics (frontend execute handlers still run, busy counter still tracked, boardPath still attached).
- [ ] After this change, arrow-key nav in the 2000-row swissarmyhammer board produces at most the same backend IPCs as today, MINUS the `perspective.list` cascade (already addressed separately but now reinforced at the source).

## Tests
- [ ] New cases in `kanban-app/ui/src/lib/command-scope.test.tsx` covering the three identity/semantics assertions above.
- [ ] Test command: `cd kanban-app/ui && npm test -- command-scope`. Expected: green.
- [ ] Full UI suite: `cd kanban-app/ui && npm test`. Expected: green.
- [ ] Manual smoke: arrow-key nav in grid view while tracking `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` — no spurious IPCs beyond `ui.setFocus` per keystroke.

## Workflow
- Use `/tdd` — write the three identity/semantics tests first (two failing on the current code, one already passing), then refactor to make them all pass.
- Land after 01KPZPY5F5HPXDKKHGKDEW6FNZ and 01KPZQDAC6P0AHTQ5F08A170H4 so those local fixes serve as additional regression nets during the change. #performance #architecture #frontend #commands