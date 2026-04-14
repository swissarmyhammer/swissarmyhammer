---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffcd80
project: expr-filter
title: 'Decouple perspective filter save from filter refresh: make refetch visibly in-flight and latest-wins'
---
## What

Today, typing in the perspective filter editor causes a cascaded, invisibly-serialized pipeline that makes the UI feel frozen while the new filter runs over a large task set. This card makes the refetch **visibly in-flight** (the global progress bar lights up) and **latest-wins** (rapid filter edits discard stale in-flight refetches instead of queueing them).

### Current flow

1. User types in `FilterEditor` → `useFilterDispatch.handleChange` debounces 300ms → dispatches `perspective.filter`.
2. `dispatch_command` IPC runs `SetFilterCmd` → `UpdatePerspective::execute` writes the perspective YAML (fast). This is tracked by the `CommandBusy` counter via `useDispatchCommand` in `kanban-app/ui/src/lib/command-scope.tsx` (`setInflightCount((c) => c + 1)` / `- 1`), so the nav-bar progress bar at `kanban-app/ui/src/components/nav-bar.tsx` lights up briefly.
3. Backend emits `entity-field-changed` for the perspective → frontend state updates → `activePerspective.filter` changes.
4. `PerspectiveContainer` (`kanban-app/ui/src/components/perspective-container.tsx`, the `useEffect` on `[activeFilter, boardPath, refreshEntities]`) calls `refreshEntities(boardPath, activeFilter)`.
5. `RustEngineContainer.refreshEntities` (`kanban-app/ui/src/components/rust-engine-container.tsx`) calls `refreshBoards(boardPath, taskFilter)` from `kanban-app/ui/src/lib/refresh.ts`, which fans out four `invoke(...)` calls including `list_entities({entityType: "task", filter})`.
6. Backend loads, enriches, and filters every task; serializes, returns.
7. Frontend `setEntitiesByType` replaces the task list → entire board re-renders.

### The two defects

**(a) The refetch is invisible.** `refreshEntities`/`refreshBoards` call `invoke` **directly**, not through `useDispatchCommand`. `CommandBusyProvider`'s counter only tracks dispatch-command IPCs. So step 5's long-running `list_entities` call does **not** increment `inflightCount`, the nav-bar progress bar does **not** show, and the app looks dead while the filter runs.

**(b) The refetch is not latest-wins.** The `useEffect` fires for every `activeFilter` change with no cancellation. If the user types quickly (autosave debounce is 300ms, but once the first `perspective.filter` lands, the cascade to step 5 cannot be cancelled), multiple back-to-back refetches of 2000+ tasks can queue and collide. Whichever `setEntitiesByType` lands last wins, but the in-between responses still cost CPU/render cycles and can briefly flash wrong results.

### Non-goals

- Speed up `list_entities` itself. That's a separate card — filter evaluation perf is its own concern and has its own profiling work to do.
- Change `perspective.filter` backend semantics. The save is already fast; it does not need decoupling at the command layer.
- Architectural rewrite. We keep the causal chain save → state → refetch; we only make it visible and cancellable.

### Approach

**(1) Busy tracking for refetches.** Extend the busy pipeline so `refreshEntities` participates in the same progress indicator as `useDispatchCommand`. Two options, pick the minimal one:

- **Option A (preferred — one counter, zero new context)**: lift `inflightCount` state up into `rust-engine-container.tsx`, or import the setter into `refreshEntities` via `useContext(CommandBusySetterContext)`, and wrap the `refreshBoards` call in the same `try { c+1 } finally { c-1 }` pattern already used by `useDispatchCommand`. This requires `CommandBusySetterContext` to be exported from `command-scope.tsx` (it is already declared, just not exported — see line 59 of `command-scope.tsx`).
- **Option B (new context)**: add a parallel `RefreshBusyContext` and have the nav-bar progress bar OR the two. More infrastructure, no real benefit over (A). Do not pick this unless (A) has a blocker.

Pick **Option A**. Export `CommandBusySetterContext` from `command-scope.tsx`, have `RustEngineContainer` read it in a hook, and wrap the body of `refreshEntities` in an increment/decrement pair. This also means the nav-bar progress bar unchanged — it already consumes `isBusy` from the same counter.

**(2) Latest-wins refetch.** Add a monotonic refetch-id ref inside `refreshEntities`. At the start of each call, capture the current id and bump it; at the end, compare against the current ref — if it changed, discard the result and do not call `setEntitiesByType`. This is the standard "stale response" guard.

```ts
const refetchIdRef = useRef(0);
const refreshEntities = useCallback(
  async (boardPath: string, taskFilter?: string): Promise<RefreshResult> => {
    activeBoardPathRef.current = boardPath;
    const myId = ++refetchIdRef.current;
    setInflightCount((c) => c + 1);
    try {
      const result = await refreshBoards(boardPath, taskFilter);
      if (myId !== refetchIdRef.current) {
        // A newer refresh started after we did; discard our result.
        return result;
      }
      if (result.entitiesByType) setEntitiesByType(result.entitiesByType);
      return result;
    } finally {
      setInflightCount((c) => c - 1);
    }
  },
  [setInflightCount],
);
```

Keep the `return result` (with no store update) on the stale path so callers that rely on the openBoards/boardData fields still get a value — just don't overwrite the entity store with stale tasks.

### Subtasks

- [x] Export `CommandBusySetterContext` from `kanban-app/ui/src/lib/command-scope.tsx` and add a public hook `useSetCommandInflight()` returning the setter, or accept the raw setter — whichever matches the prevailing pattern (other context setters in this file use the `useContext` direct-read pattern; follow it). **Done**: added `useSetCommandInflight()` hook (mirrors the existing `useCommandBusy()` reader hook).
- [x] In `kanban-app/ui/src/components/rust-engine-container.tsx`, read the setter inside `RustEngineContainer` and wrap `refreshEntities` with the `try { setInflightCount(c+1) } finally { setInflightCount(c-1) }` pattern so the progress bar lights up for the duration of `refreshBoards`.
- [x] Add a `refetchIdRef` latest-wins guard to `refreshEntities` that discards stale responses without calling `setEntitiesByType`.
- [x] Confirm `PerspectiveContainer`'s `useEffect` (`kanban-app/ui/src/components/perspective-container.tsx`) does not need changes — the latest-wins guard sits entirely inside `refreshEntities`, so rapid `activeFilter` changes get correctly collapsed at the single point that matters. **Confirmed**: no PerspectiveContainer changes needed.

## Acceptance Criteria

- [x] While a `refreshEntities` call is in flight (filter applied on a board with many tasks), the nav-bar progress bar at `kanban-app/ui/src/components/nav-bar.tsx` is visible (`role="progressbar"` element rendered).
- [x] When the user types `$a`, `$ab`, `$abc` in rapid succession (each triggering a filter save and a cascading `refreshEntities`), only the **final** `setEntitiesByType` lands on the store. Intermediate refetches either return early or are discarded before their state update.
- [x] The `perspective.filter` dispatch itself remains unchanged — its own busy tracking (via `useDispatchCommand`) still works; this card adds a **second** participant in the same counter, not a replacement.
- [x] No regression in `list_entities` semantics or backend perspective persistence. No backend changes ship in this card.

## Tests

- [x] **Latest-wins unit test** — `kanban-app/ui/src/components/rust-engine-container.test.tsx`. Mock `refreshBoards` to return two deferred promises (call 1 resolves *after* call 2). Call `refreshEntities` twice in sequence. Assert that `setEntitiesByType` is only called once with the second call's data; the first call's delayed resolution must not touch the store. Use a testing-library `renderHook` or the existing test's component-level assertion pattern — follow whichever is already present in `rust-engine-container.test.tsx`. **Added**: `refreshEntities: stale (out-of-order) responses do not overwrite the store` and `refreshEntities: stale call still returns its result to the caller`.
- [x] **Busy-tracking unit test** — same file. Mock `refreshBoards` with a deferred promise. Call `refreshEntities`. Assert the `CommandBusy` context reports `isBusy === true` while pending, and `false` after the promise resolves. Use the `CommandBusyProvider` wrapper already imported by the existing tests. **Added**: three tests covering normal resolution, rejection, and overlapping calls.
- [x] **Integration test (optional, if the existing test harness supports it)** — `kanban-app/ui/src/components/perspective-container.test.tsx`. Render a `PerspectiveContainer` with a changing `activeFilter` and assert the nav-bar `[role="progressbar"]` appears during the refetch window. If the existing harness doesn't render the nav bar in this test file, skip this subtask — the unit test above is sufficient. **Skipped**: existing perspective-container test harness mocks out `useRefreshEntities` entirely; the unit tests added in `rust-engine-container.test.tsx` cover this cleanly.
- [x] **Regression**: `npx vitest run src/components/rust-engine-container.test.tsx src/components/perspective-container.test.tsx src/components/nav-bar.test.tsx src/components/filter-editor.test.tsx` — all green. These are the nearest neighbors to the commit path and must not regress. **Verified**: 48/48 passing (47 pre-review + 1 new App-shape integration test).
- [x] **Full suite**: `cd kanban-app/ui && pnpm test` — full frontend suite green. `cargo nextest run -p swissarmyhammer-kanban -p kanban-app` — full backend suite green (no backend changes in this card, so this is a safety net). **Verified**: 1041 frontend tests + 2 skipped + 1158 backend tests, all green.

## Workflow

Use `/tdd`. Write the latest-wins failing test first — it should assert that when `refreshBoards` resolves out-of-order, only the newer call's data lands in the entity store. Watch it fail on the current implementation (where both resolutions update the store). Then add the `refetchIdRef` guard. Add the busy-tracking test next, confirm it fails (current implementation bypasses the counter), then wire the `setInflightCount` increments. Do not touch `PerspectiveContainer` unless the tests drive you there.

## Notes / related

- This card is the **visibility + cancellation** half of the fix described in my conversation with the user about filter-save regressions. The other half — profiling and speeding up `list_entities` for 2000-task boards — is a separate card (filed alongside).
- The refetch is currently serialized through React state (perspective state → useEffect → refetch). That's fine; this card does not try to break that causal chain. It only (a) surfaces the cost and (b) discards intermediate responses when the user is still typing.
- `refreshBoards` itself has no cancellation support (it calls four `invoke()`s in a `Promise.all`). We cannot cancel the backend `list_entities` calls mid-flight without adding AbortController plumbing across the Tauri IPC boundary — out of scope for this card. The latest-wins guard is cheaper and achieves the same user-visible behavior (final value wins, no flicker).

## Review Findings (2026-04-13 14:30)

### Blockers

- [x] `kanban-app/ui/src/components/rust-engine-container.tsx:186` — `useSetCommandInflight()` is called inside `RustEngineContainer`, but in production the only mount of `CommandBusyProvider` is inside `WindowContainer` (`kanban-app/ui/src/components/window-container.tsx:403`), and `App.tsx:39-78` renders `<RustEngineContainer>` **outside** `<WindowContainer>`. There is no `CommandBusyProvider` ancestor of the hook call, so the setter returned is the **no-op default** (`command-scope.tsx:61`). At runtime `setInflightCount((c) => c + 1)` does nothing, the `CommandBusy` counter never increments for refetches, and the nav-bar progress bar does **not** light up — which is the first of the two defects this card is supposed to fix. The busy-tracking unit tests pass only because they explicitly wrap `<RustEngineContainer>` in `<CommandBusyProvider>` in the test harness (`rust-engine-container.test.tsx:888, 934, 980`) — a tree shape that does not match `App.tsx`. Fix: move `CommandBusyProvider` out of `WindowContainer` and up to wrap `RustEngineContainer` in `App.tsx` (or, equivalently, push `CommandBusyProvider` into the outermost container so both the dispatch-command writer inside `WindowContainer` and the refresh writer inside `RustEngineContainer` share the same provider). Add an integration test that renders the real `App`-shaped tree (`<RustEngineContainer><...><CommandBusyProvider>` order) and asserts the progress bar lights up during a `refreshEntities` call — that is the test that would have caught this. **Fixed**: hoisted `CommandBusyProvider` out of `WindowContainer` and into `App.tsx` so it wraps `RustEngineContainer` — both the dispatch-command writer (inside `WindowContainer`) and the refresh writer (inside `RustEngineContainer`) now participate in the same counter. Added new integration test `refreshEntities: nav-bar progress bar is reachable in the production App tree shape` in `rust-engine-container.test.tsx` that renders the production `<CommandBusyProvider><RustEngineContainer>...` tree and asserts a `role="progressbar"` element appears during a refetch — this test would have failed on the broken wiring.

### Warnings

- [x] `kanban-app/ui/src/lib/command-scope.tsx:110-113` — The JSDoc says "Returns a no-op setter when no `CommandBusyProvider` is mounted, so callers outside the provider tree (tests, isolated probes) do not need to special-case the absence of the provider." In practice, the only in-tree caller added by this card (`RustEngineContainer`) **is** outside the provider tree in production, and the no-op fallback silently masks the bug. Either (a) after fixing the provider placement above, tighten the JSDoc to reflect the intended production contract ("every real call site must sit inside a `CommandBusyProvider`; the default is a no-op only to keep isolated unit tests ergonomic"), or (b) throw/warn in dev when the default setter is hit from a non-test context so the next wiring regression is loud. **Both**: tightened JSDoc to state the production contract explicitly, AND added a dev-mode one-shot `console.warn` when the default no-op setter is invoked. The warning is gated on `import.meta.env.DEV` so production bundles stay silent; the sentinel `NOOP_INFLIGHT_SETTER` reference is compared against the resolved context value to detect the no-provider case without affecting real call paths.
- [x] `kanban-app/ui/src/components/rust-engine-container.test.tsx:888,934,980` — The busy-tracking tests wrap `<RustEngineContainer>` *inside* `<CommandBusyProvider>`, which is the inverse of the production `App.tsx` tree. This is why they green under a broken production wiring. Once the provider placement is fixed, add at least one integration-level assertion that renders the `App`-shaped tree (or a close shim) and proves the counter is actually reachable from the refetch call site — not just from a synthetic wrapper. **Done**: with the provider placement now fixed in `App.tsx` (`<CommandBusyProvider><RustEngineContainer>`), the existing busy-tracking tests' wrapper order actually matches the production tree. The new integration test `refreshEntities: nav-bar progress bar is reachable in the production App tree shape` asserts the counter is reachable end-to-end by checking for the `role="progressbar"` element (nav-bar's DOM contract) rather than the internal `isBusy` boolean.

### Nits

- [x] `kanban-app/ui/src/components/rust-engine-container.tsx:214-237` — The block comment at lines 198-213 is good, but the inline comment inside the stale branch ("A newer refresh started after we did; discard our result so the entity store reflects only the latest caller's data. We still return `result` so non-entity consumers…") duplicates what the doc comment already says. Either drop the inline comment or trim the doc-comment's "Latest-wins guard" bullet to a one-liner pointer. Minor readability nit only. **Done**: collapsed the inline comment to a single pointer back to the block comment.
- [x] `kanban-app/ui/src/components/rust-engine-container.test.tsx:16-20` — The `realRefreshBoardsHolder` indirection (a mutable holder populated during `vi.mock`'s async factory) is cleverer than it needs to be. A simpler pattern would be `mockRefreshBoards.mockImplementation(actual.refreshBoards)` at the top of the mock factory, or per-test overrides without the holder. Not worth a change by itself, but if you touch this file again consider simplifying. **Done**: replaced the `{ fn }` holder and the args-splat wrapper with a typed `{ current }` ref and a direct `mockImplementation(realRefreshBoards.current!)` in `beforeEach`. Cannot eliminate the holder entirely because `vi.clearAllMocks()` in `beforeEach` wipes implementations set inside the `vi.mock` factory, so a captured reference is still needed.
