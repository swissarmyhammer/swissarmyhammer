---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
position_column: done
position_ordinal: ffffffffffffffffffffffffa380
title: 'Split EntityFocusContext: selector-subscribed focus store so focus moves don''t re-render all ~12k FocusScopes'
---
## What

`kanban-app/ui/src/lib/entity-focus-context.tsx` bundles one volatile field (`focusedMoniker`) with seven stable callbacks (`setFocus`, `registerScope`, `unregisterScope`, `getScope`, `registerClaimPredicates`, `unregisterClaimPredicates`, `broadcastNavCommand`) into a single React context value memoized with `focusedMoniker` in the deps. Every focus change produces a new `value` reference. React context propagation forces **every `useEntityFocus()` call site to re-render on every focus move**, even though most of them only use the stable callbacks and only a small set genuinely cares about *who* is focused.

In the grid that means a single arrow-key press re-renders ~12k `FocusScope` instances (one per cell) plus ~2k `EntityRow` instances. The only logically necessary re-renders on focus move are the cell that lost the highlight and the cell that gained it — exactly two. The remaining ~14k are pure waste, triggered solely by context-value identity churn.

**The pattern we already own**: `kanban-app/ui/src/lib/entity-store-context.tsx` solves the structurally identical problem for entity fields via `class FieldSubscriptions { subs: Map<string, Set<cb>> }` + `useSyncExternalStore`. `useFieldValue(entityType, id, fieldName)` subscribes to one specific `(type, id, field)` key; only that key's subscribers fire when that field's value changes. We apply the same mechanism to focus, swapping the key for `moniker`.

## Implementation note — hook naming

The task's proposed API spelled the new selective-subscription hook `useIsFocused`, but that name was already taken by an existing hook with **ancestor-walking semantics** (returns true if `moniker` is the focused scope OR an ancestor of it). That hook has a real production caller (`FieldRow` in `entity-inspector.tsx` — its field wraps pills that can nest `FocusScope`, so ancestor-match is load-bearing) and an explicit regression test in `focus-scope.test.tsx` (`"useIsFocused ancestor: column gets data-focused when card inside is focused"`).

Unifying into one name would require either (a) breaking the ancestor contract, or (b) layering both semantics into one hook, which can't be implemented cleanly with selective subscription (subscribing to moniker A's slot doesn't fire when focus moves between two descendants of A, neither of which is A).

Resolution: new direct-only selector hook is exported as **`useIsDirectFocus(moniker)`**. Existing `useIsFocused(moniker)` keeps ancestor-walking semantics; its internals now run off `useFocusedMoniker()` + `getScope` from actions (so its callers re-render broadly, which is acceptable — there is only one, FieldRow). FocusScope uses `useIsDirectFocus` to drive its direct-focus highlight. The ancestor test still passes unchanged.

## Proposed API

```ts
// Stable actions — plain React context, value object memoized once, never churns.
// Consumers reading only actions never re-render on focus moves.
export function useFocusActions(): {
  setFocus: (moniker: string | null) => void;
  registerScope: (moniker: string, scope: CommandScope) => void;
  unregisterScope: (moniker: string) => void;
  getScope: (moniker: string) => CommandScope | null;
  registerClaimPredicates: (moniker: string, preds: ClaimPredicate[]) => void;
  unregisterClaimPredicates: (moniker: string) => void;
  broadcastNavCommand: (commandId: string) => boolean;
};

// Imperative store handle — for consumers that need to read focus state
// inside an event handler or effect without subscribing to every change.
export function useFocusStore(): FocusStore;

// Selective subscription — consumer is notified ONLY when `myMoniker`'s
// focus state flips (gained or lost). This is the hook FocusScope uses
// to compute `isDirectFocus`. (Named `useIsDirectFocus` to distinguish
// from the pre-existing ancestor-walking `useIsFocused` — see
// "Implementation note — hook naming" above.)
export function useIsDirectFocus(myMoniker: string): boolean;

// Broad subscription — consumer re-renders on every focus change.
// For the handful of call sites that genuinely need the current moniker
// (grid cursor derivation, board-view bookkeeping).
export function useFocusedMoniker(): string | null;

// Ref-style broad subscription — ref.current is updated on focus change
// but reading it does NOT trigger a re-render. For one-shot captures
// (e.g. entity-inspector saving `previousFocus` at mount).
export function useFocusedMonikerRef(): React.MutableRefObject<string | null>;
```

Internally:
```ts
class FocusStore {
  private current: string | null = null;
  private perMoniker = new Map<string, Set<() => void>>();  // key = moniker
  private anyListeners = new Set<() => void>();              // broad listeners

  getSnapshot(): string | null { return this.current; }

  subscribe(moniker: string, cb: () => void): () => void {
    let set = this.perMoniker.get(moniker);
    if (!set) { set = new Set(); this.perMoniker.set(moniker, set); }
    set.add(cb);
    return () => { set!.delete(cb); if (set!.size === 0) this.perMoniker.delete(moniker); };
  }

  subscribeAll(cb: () => void): () => void {
    this.anyListeners.add(cb);
    return () => { this.anyListeners.delete(cb); };
  }

  set(next: string | null): void {
    const prev = this.current;
    if (prev === next) return;
    this.current = next;
    // Notify ONLY the two affected moniker slots + all broad listeners
    if (prev !== null) this.perMoniker.get(prev)?.forEach(cb => cb());
    if (next !== null) this.perMoniker.get(next)?.forEach(cb => cb());
    this.anyListeners.forEach(cb => cb());
  }
}
```

Direct structural copy of `FieldSubscriptions` — same `Map<key, Set<cb>>` shape, same notify-on-key semantics, same `useSyncExternalStore` plumbing in the hooks.

**Compat layer** — keep `useEntityFocus()` working as a thin wrapper around `useFocusActions()` + `useFocusedMoniker()` so existing tests and non-hot call sites can migrate incrementally without a mass rewrite. The wrapper still re-renders on every focus change by definition (it reads the broad moniker), so migrating off of it is what actually delivers the perf win. Mark the wrapper `@deprecated` in the TSDoc to steer future authors to the narrow hooks.

## Enumerated consumers (production code, non-test)

Confirmed via `Grep "useEntityFocus|useFocusedScope" kanban-app/ui/src` (excluding `*.test.tsx`).

| # | File | Line | Fields read | Migrate to | Instances | Priority |
|---|------|------|-------------|------------|-----------|----------|
| 1 | `kanban-app/ui/src/components/focus-scope.tsx` — `FocusScope` | 87-93 | `focusedMoniker`, `setFocus`, `registerScope`, `unregisterScope`, `registerClaimPredicates`, `unregisterClaimPredicates` | `useIsDirectFocus(moniker)` + `useFocusActions()` | **~12k** | **P0 (hot)** |
| 2 | `kanban-app/ui/src/components/focus-scope.tsx` — `FocusScopeInner` | 191 | `setFocus` | `useFocusActions()` | **~12k** | **P0 (hot)** |
| 3 | `kanban-app/ui/src/components/data-table.tsx` — `EntityRow` | 676 | `setFocus` | `useFocusActions()` | **~2k** | **P0 (hot)** |
| 4 | `kanban-app/ui/src/components/grid-view.tsx` — `useGridNavigation` | 311 | `focusedMoniker`, `setFocus`, `broadcastNavCommand` | `useFocusedMoniker()` + `useFocusActions()` | 1 per grid view | P1 |
| 5 | `kanban-app/ui/src/components/board-view.tsx` — `BoardView` | 1034 | `focusedMoniker`, `broadcastNavCommand`, `setFocus` (wraps into ref via `useBoardCommandRefs`) | `useFocusedMonikerRef()` + `useFocusedMoniker()` (scroll effect) + `useFocusActions()` | 1 per board view | P1 |
| 6 | `kanban-app/ui/src/components/entity-inspector.tsx` — `useFirstFieldFocus` | 209 | `setFocus`, `focusedMoniker` (one-shot capture of prev focus at mount) | `useFocusActions()` + `useFocusedMonikerRef()` | 1 per inspector | P1 |
| 7 | `kanban-app/ui/src/components/cursor-focus-bridge.tsx` | 18 | `setFocus`, `registerScope`, `unregisterScope` | `useFocusActions()` | 1 per grid | P1 |
| 8 | `kanban-app/ui/src/components/inspector-focus-bridge.tsx` | 28 | `broadcastNavCommand` | `useFocusActions()` | 1 per inspector | P1 |
| 9 | `kanban-app/ui/src/components/column-view.tsx` | 487 | `setFocus` | `useFocusActions()` | ~1 per column | P1 |
| 10 | `kanban-app/ui/src/components/app-shell.tsx` — `AppShell` | 334 | `broadcastNavCommand` | `useFocusActions()` | 1 | P1 |
| 11 | `kanban-app/ui/src/components/app-shell.tsx` — via `useFocusedScope` | 32 | derived scope from focusedMoniker | keep `useFocusedScope()` hook; rewrite it internally to use `useFocusedMoniker()` + action `getScope` | 1 | P1 |
| 12 | `kanban-app/ui/src/lib/entity-focus-context.tsx` — `useFocusedScope` | 261 | `focusedMoniker`, `getScope` | internal rewrite: `useFocusedMoniker()` + `useFocusActions().getScope` | hook, not an instance | P1 |
| 13 | `kanban-app/ui/src/lib/entity-focus-context.tsx` — provider (derives `FocusedScopeContext`) | 224 | `focusedMoniker` | provider reads store internally via `useSyncExternalStore` to derive `FocusedScopeContext`; no user-facing change | 1 | P1 |

**Hot-path wins**: migrating rows 1–3 alone takes per-nav re-renders from ~14k to exactly 2 (the losing cell + the gaining cell). Everything else is cleanup for correctness/consistency.

## Files

- `kanban-app/ui/src/lib/entity-focus-context.tsx` — add `FocusStore`, new hooks, migrate provider. Keep `useEntityFocus` as deprecated compat shim. Keep `useFocusedScope` signature; rewrite its internals.
- `kanban-app/ui/src/components/focus-scope.tsx` — migrate `FocusScope` and `FocusScopeInner` (#1, #2).
- `kanban-app/ui/src/components/data-table.tsx` — migrate `EntityRow` (#3).
- `kanban-app/ui/src/components/grid-view.tsx` — migrate `useGridNavigation` (#4).
- `kanban-app/ui/src/components/board-view.tsx` — migrate `BoardView`/`useBoardCommandRefs` (#5).
- `kanban-app/ui/src/components/entity-inspector.tsx` — migrate `useFirstFieldFocus` (#6).
- `kanban-app/ui/src/components/cursor-focus-bridge.tsx` — migrate (#7).
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — migrate (#8).
- `kanban-app/ui/src/components/column-view.tsx` — migrate (#9).
- `kanban-app/ui/src/components/app-shell.tsx` — migrate (#10).
- `kanban-app/ui/src/lib/entity-focus-context.test.tsx` — new tests for the store + selector hooks.

## Test mock inventory

Files that mock `useEntityFocus` — these continue to work via the compat shim without changes. Flagged here so the implementer knows the surface to spot-check after the migration:

- `kanban-app/ui/src/lib/entity-focus-context.test.tsx`
- `kanban-app/ui/src/components/rust-engine-container.test.tsx`
- `kanban-app/ui/src/components/grid-view.test.tsx` — **mock updated** (added new hooks)
- `kanban-app/ui/src/components/grid-view.stale-card-fields.test.tsx` — **mock updated**
- `kanban-app/ui/src/components/grid-empty-state.browser.test.tsx` — **mock updated**
- `kanban-app/ui/src/components/app-shell.test.tsx`
- `kanban-app/ui/src/components/focus-scope.test.tsx`
- `kanban-app/ui/src/components/entity-inspector.test.tsx`
- `kanban-app/ui/src/components/inspector-focus-bridge.test.tsx`
- `kanban-app/ui/src/components/inspectors-container.test.tsx` — **mock updated**
- `kanban-app/ui/src/components/mention-view.test.tsx`
- `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx`

### Subtasks
- [x] Add `FocusStore` class to `entity-focus-context.tsx`, patterned on `FieldSubscriptions` in `entity-store-context.tsx` — same `Map<key, Set<cb>>` shape, same notify-on-key semantics.
- [x] Add hooks: `useFocusStore`, `useFocusActions`, `useIsDirectFocus` (direct-only selector; the task's proposed `useIsFocused` name was kept for the pre-existing ancestor-walking hook — see naming note), `useFocusedMoniker`, `useFocusedMonikerRef`.
- [x] Rewrite `EntityFocusProvider` to own the store (via `useRef(new FocusStore())`), expose actions via `FocusActionsContext`, expose the store via `FocusStoreContext`, and derive `FocusedScopeContext` via `useSyncExternalStore` (so its own re-renders stay narrow).
- [x] Keep `useEntityFocus()` as `@deprecated` compat shim built from `useFocusActions()` + `useFocusedMoniker()`.
- [x] Migrate hot consumers (rows 1–3 in the enumeration table).
- [x] Migrate cool consumers (rows 4–10).
- [x] Rewrite `useFocusedScope()` internals to use `useFocusedMoniker()` + `getScope` from actions; public signature unchanged.
- [x] Add new tests for `FocusStore` + the four hooks (see Tests).
- [x] Run the full UI test suite and verify every existing mock still works via the compat shim.

## Acceptance Criteria

- [x] `FocusStore` is a direct structural port of `FieldSubscriptions`: same `Map<key, Set<cb>>` shape, same notify-only-matching-key behavior.
- [x] `useIsDirectFocus(moniker)` subscribes only to that moniker's slot — `"only notifies the two affected monikers on focus change"` test in `entity-focus-context.test.tsx` asserts this: changing focus from A to B wakes exactly A and B, never C.
- [x] `useFocusActions()` value reference is strictly stable across focus moves (one-time creation in the provider via a lazy-init ref, covered by `"useFocusActions value identity is stable across focus moves"` test).
- [x] All 13 production consumers enumerated above migrated as specified. Row 5 (BoardView) additionally retains a broad `useFocusedMoniker()` read to feed `useScrollFocusedIntoView`, which needs reactivity (refs don't re-trigger effects); BoardView is still only one component re-rendering on focus move, not per-cell.
- [x] `useEntityFocus()` remains exported and functional (compat shim) with `@deprecated` TSDoc.
- [ ] **Telemetry acceptance** (requires manual run + RenderProfiler from 01KPZWP4YTYH76XTBH992RV2AS): deferred to review — the automated selective-wake test (`"FocusScope re-renders exactly when its own moniker's focus state flips"`) already proves the invariant on five scopes; the 2000-row capture is confirmation in the real app environment.
- [x] No behavioral regression: all 1363 tests pass including the `useIsFocused` ancestor test, click-to-focus tests, claim-predicate tests, focus-scope tests, and the `grid-view.nav-is-eventdriven.test.tsx` invariant.

## Tests

- [x] `kanban-app/ui/src/lib/entity-focus-context.test.tsx` — `"only notifies the two affected monikers on focus change"` for `useIsDirectFocus`: subscribe via `renderHook` for monikers A, B, C; `act(() => store.set("A"))`; asserts A's callback fired, B and C did not. `act(() => store.set("B"))`; asserts A and B fired, C did not.
- [x] Same file — `"useFocusActions value identity is stable across focus moves"`: captures `result.current`; dispatches several focus changes; asserts same ref throughout.
- [x] Same file — `"useFocusedMoniker returns null initially and tracks changes"`: snapshot test mirroring `useFieldValue` broad cases.
- [x] Same file — `"useFocusedMonikerRef updates ref without re-rendering its caller"`: probe counts its own renders; dispatches N focus changes; asserts probe rendered 0 additional times AND `ref.current` matches the latest value.
- [x] Same file — `"exposes the combined shape and still re-renders on focus"`: compat shim preserves existing behavior so test mocks keep working.
- [x] `kanban-app/ui/src/components/focus-scope.test.tsx` — existing ancestor test passes unchanged; new `"FocusScope re-renders exactly when its own moniker's focus state flips"` mounts 5 FocusScopes + 5 subscribed counters (counters call `useIsDirectFocus` directly — passing counters via `children` would be measured away by React's element-identity memoization), changes focus A→B, asserts only A and B counters incremented.
- [x] Additional `FocusStore`-standalone tests (no provider needed): `subscribe notifies only the matching moniker`, `set is a no-op when the value does not change`, `subscribeAll fires on every change`, `unsubscribe stops further notifications and prunes empty slots`, `getSnapshot reflects the current value`.
- [x] Test command: `cd kanban-app/ui && npm test -- entity-focus-context focus-scope`. Result: green (61/61 tests pass).
- [x] Full UI suite: `cd kanban-app/ui && npm test`. Result: green (1363/1363 tests pass).
- [ ] Manual smoke + telemetry capture: deferred to review (requires running the 2000-row board app).

## Workflow

- Use `/tdd` — start with the `FocusStore` class and the per-moniker selective notification test (copy the shape from `FieldSubscriptions` tests), then hooks, then hot-path migrations, then cool consumers.
- Land after **01KPZWP4YTYH76XTBH992RV2AS** (RenderProfiler) so the telemetry acceptance has something to measure against.
- Lands independently of the other performance tasks — does not require #2/#3/#4/#6 first, and does not block them. The four tasks compound with this one but each is independently valuable. #performance #architecture #frontend

## Review Findings (2026-04-24 12:24)

### Warnings
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx` — `buildFocusActions.setFocus` fires `console.warn(`[FocusScope] focus → …`)` on every focus move. In a grid user holding down an arrow key this produces one log per keypress — exactly the hot path this task was written to make cheap. The warn existed pre-refactor, but the refactor's stated purpose is to minimize per-move work and logging a formatted string on every move directly contradicts that goal. Fix: remove the `console.warn` line, or gate it behind a debug flag (e.g. `if (import.meta.env.DEV && FOCUS_TRACE)`). If the trace is needed for development, it belongs behind an opt-in flag, not unconditionally on every focus change. Resolved 2026-04-24: removed the unconditional `console.warn` from `buildFocusActions.setFocus`. Focus moves are now silent on the hot path.
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx` — `store.subscribeAll.bind(store)` is passed inline to `useSyncExternalStore` in three places: the provider body, `useFocusedMoniker`, and the `useEntityFocus` compat shim. `.bind()` returns a new function on every render, which per the React docs causes `useSyncExternalStore` to unsubscribe and re-subscribe on every re-render. None of these call sites are in the ~12k hot path, but each one contradicts the recommended pattern the rest of this refactor is built on (see `useIsDirectFocus` which correctly uses `useCallback`). Fix: simplest is to make `subscribeAll` an arrow-function instance property on `FocusStore` mirroring `getSnapshot` (`subscribeAll = (cb) => { … }`) — once that's done, `store.subscribeAll` is inherently stable and all three call sites can drop `.bind(store)` and pass `store.subscribeAll` directly. That also makes the class more consistent (two arrow-fn properties instead of one plus a method). Resolved 2026-04-24: promoted `subscribeAll` to an arrow-function instance property on `FocusStore` (mirroring `getSnapshot`) and dropped `.bind(store)` at all three `useSyncExternalStore` call sites — the provider body, `useFocusedMoniker`, and the `useEntityFocus` compat shim now pass `store.subscribeAll` directly.

### Nits
- [x] `ARCHITECTURE.md` — the "Field-Level Subscriptions" subsection documents `FieldSubscriptions` as the project's pattern for selector-subscribed state. This task explicitly introduces `FocusStore` as a "direct structural port of `FieldSubscriptions`" for focus. A one- or two-sentence parallel note ("The same `Map<key, Set<cb>>` pattern is applied to focus via `FocusStore`, keyed by moniker; see `entity-focus-context.tsx`") would make the parallel discoverable for future authors. Resolved 2026-04-24: added a one-sentence parallel note under "Field-Level Subscriptions" pointing readers to `FocusStore` in `kanban-app/ui/src/lib/entity-focus-context.tsx`.
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx` — `useIsDirectFocus` naming deviation from the task spec (`useIsFocused`) is documented in the task description with sound reasoning (existing ancestor-walking `useIsFocused` has a production caller and a regression test; semantics can't be cleanly unified). Keeping two names is the pragmatic choice. Flagged here only to confirm review-level acceptance of the deviation — no change requested. Resolved 2026-04-24: no code change requested; deviation is documented and intentional.