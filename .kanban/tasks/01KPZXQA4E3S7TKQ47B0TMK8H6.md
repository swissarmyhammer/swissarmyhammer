---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
position_column: todo
position_ordinal: ff9480
title: 'Split EntityFocusContext: selector-subscribed focus store so focus moves don''t re-render all ~12k FocusScopes'
---
## What

`kanban-app/ui/src/lib/entity-focus-context.tsx` bundles one volatile field (`focusedMoniker`) with seven stable callbacks (`setFocus`, `registerScope`, `unregisterScope`, `getScope`, `registerClaimPredicates`, `unregisterClaimPredicates`, `broadcastNavCommand`) into a single React context value memoized with `focusedMoniker` in the deps. Every focus change produces a new `value` reference. React context propagation forces **every `useEntityFocus()` call site to re-render on every focus move**, even though most of them only use the stable callbacks and only a small set genuinely cares about *who* is focused.

In the grid that means a single arrow-key press re-renders ~12k `FocusScope` instances (one per cell) plus ~2k `EntityRow` instances. The only logically necessary re-renders on focus move are the cell that lost the highlight and the cell that gained it — exactly two. The remaining ~14k are pure waste, triggered solely by context-value identity churn.

**The pattern we already own**: `kanban-app/ui/src/lib/entity-store-context.tsx` solves the structurally identical problem for entity fields via `class FieldSubscriptions { subs: Map<string, Set<cb>> }` + `useSyncExternalStore`. `useFieldValue(entityType, id, fieldName)` subscribes to one specific `(type, id, field)` key; only that key's subscribers fire when that field's value changes. We apply the same mechanism to focus, swapping the key for `moniker`.

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
// to compute `isDirectFocus`.
export function useIsFocused(myMoniker: string): boolean;

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
| 1 | `kanban-app/ui/src/components/focus-scope.tsx` — `FocusScope` | 87-93 | `focusedMoniker`, `setFocus`, `registerScope`, `unregisterScope`, `registerClaimPredicates`, `unregisterClaimPredicates` | `useIsFocused(moniker)` + `useFocusActions()` | **~12k** | **P0 (hot)** |
| 2 | `kanban-app/ui/src/components/focus-scope.tsx` — `FocusScopeInner` | 191 | `setFocus` | `useFocusActions()` | **~12k** | **P0 (hot)** |
| 3 | `kanban-app/ui/src/components/data-table.tsx` — `EntityRow` | 676 | `setFocus` | `useFocusActions()` | **~2k** | **P0 (hot)** |
| 4 | `kanban-app/ui/src/components/grid-view.tsx` — `useGridNavigation` | 311 | `focusedMoniker`, `setFocus`, `broadcastNavCommand` | `useFocusedMoniker()` + `useFocusActions()` | 1 per grid view | P1 |
| 5 | `kanban-app/ui/src/components/board-view.tsx` — `BoardView` | 1034 | `focusedMoniker`, `broadcastNavCommand`, `setFocus` (wraps into ref via `useBoardCommandRefs`) | `useFocusedMonikerRef()` + `useFocusActions()` | 1 per board view | P1 |
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
- `kanban-app/ui/src/components/grid-view.test.tsx`
- `kanban-app/ui/src/components/grid-view.stale-card-fields.test.tsx`
- `kanban-app/ui/src/components/grid-empty-state.browser.test.tsx`
- `kanban-app/ui/src/components/app-shell.test.tsx`
- `kanban-app/ui/src/components/focus-scope.test.tsx`
- `kanban-app/ui/src/components/entity-inspector.test.tsx`
- `kanban-app/ui/src/components/inspector-focus-bridge.test.tsx`
- `kanban-app/ui/src/components/inspectors-container.test.tsx`
- `kanban-app/ui/src/components/mention-view.test.tsx`
- `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx`

### Subtasks
- [ ] Add `FocusStore` class to `entity-focus-context.tsx`, patterned on `FieldSubscriptions` in `entity-store-context.tsx` — same `Map<key, Set<cb>>` shape, same notify-on-key semantics.
- [ ] Add hooks: `useFocusStore`, `useFocusActions`, `useIsFocused`, `useFocusedMoniker`, `useFocusedMonikerRef`.
- [ ] Rewrite `EntityFocusProvider` to own the store (via `useRef(new FocusStore())`), expose actions via `FocusActionsContext`, expose the store via `FocusStoreContext`, and derive `FocusedScopeContext` via `useSyncExternalStore` (so its own re-renders stay narrow).
- [ ] Keep `useEntityFocus()` as `@deprecated` compat shim built from `useFocusActions()` + `useFocusedMoniker()`.
- [ ] Migrate hot consumers (rows 1–3 in the enumeration table).
- [ ] Migrate cool consumers (rows 4–10).
- [ ] Rewrite `useFocusedScope()` internals to use `useFocusedMoniker()` + `getScope` from actions; public signature unchanged.
- [ ] Add new tests for `FocusStore` + the four hooks (see Tests).
- [ ] Run the full UI test suite and verify every existing mock still works via the compat shim.

## Acceptance Criteria

- [ ] `FocusStore` is a direct structural port of `FieldSubscriptions`: same `Map<key, Set<cb>>` shape, same notify-only-matching-key behavior.
- [ ] `useIsFocused(moniker)` subscribes only to that moniker's slot — assert via the test described below that changing focus from A to B fires exactly the subscribers for A and B, and nothing else.
- [ ] `useFocusActions()` value reference is strictly stable across focus moves (one-time creation in the provider).
- [ ] All 13 production consumers enumerated above migrated as specified, or explicitly scoped out (with justification) in the PR description.
- [ ] `useEntityFocus()` remains exported and functional (compat shim) with `@deprecated` TSDoc.
- [ ] **Telemetry acceptance** (requires the RenderProfiler from 01KPZWP4YTYH76XTBH992RV2AS): with `<RenderProfiler id="view-body">` wrapping `ViewContainer`, 10 arrow-key presses in the 2000-row swissarmyhammer board produce `u≤20` (2 per press for the two affected cells, plus a handful for grid-nav bookkeeping). Before the change, `u` increases by roughly the visible-cell count per press. Capture both snapshots in the PR description.
- [ ] No behavioral regression: focus bar still renders on the focused cell; click-to-focus still works; nav commands still claim focus per the existing ClaimPredicate logic; `FocusedScopeContext` still updates so `useDispatchCommand` sees the current focused scope.

## Tests

- [ ] `kanban-app/ui/src/lib/entity-focus-context.test.tsx` — `"useIsFocused only notifies the two affected monikers on focus change"`: subscribe via `renderHook` for monikers A, B, C; `act(() => store.set("A"))`; assert A's callback fired, B and C did not. `act(() => store.set("B"))`; assert A and B fired (A because it lost focus, B because it gained), C did not.
- [ ] Same file — `"useFocusActions value identity is stable across focus moves"`: `renderHook(() => useFocusActions())`; capture `result.current`; dispatch several focus changes; assert `result.current === firstRef` throughout.
- [ ] Same file — `"useFocusedMoniker returns the current moniker and re-renders on change"`: straightforward snapshot test mirroring `useFieldValue` broad cases.
- [ ] Same file — `"useFocusedMonikerRef updates ref without re-rendering"`: render a probe that counts its own render calls; dispatch N focus changes; assert the probe re-rendered 0 times AND `ref.current` matches the latest value.
- [ ] Same file — `"compat shim useEntityFocus exposes combined shape and still re-renders on focus"`: ensures the compat layer preserves existing behavior so test mocks keep working.
- [ ] `kanban-app/ui/src/components/focus-scope.test.tsx` — existing tests pass unchanged through the compat shim during migration; after migration, add `"FocusScope re-renders exactly when its own moniker's focus state flips"`: mount ~5 FocusScopes; wrap each in a render counter; change focus A→B; assert only A and B's counters incremented.
- [ ] Test command: `cd kanban-app/ui && npm test -- entity-focus-context focus-scope`. Expected: green.
- [ ] Full UI suite: `cd kanban-app/ui && npm test`. Expected: green (no regressions via the compat shim).
- [ ] Manual smoke + telemetry capture: run the 2000-row board, arrow keys, save the `[profile] view-body` before/after snapshot in the PR description.

## Workflow

- Use `/tdd` — start with the `FocusStore` class and the per-moniker selective notification test (copy the shape from `FieldSubscriptions` tests), then hooks, then hot-path migrations, then cool consumers.
- Land after **01KPZWP4YTYH76XTBH992RV2AS** (RenderProfiler) so the telemetry acceptance has something to measure against.
- Lands independently of the other performance tasks — does not require #2/#3/#4/#6 first, and does not block them. The four tasks compound with this one but each is independently valuable. #performance #architecture #frontend