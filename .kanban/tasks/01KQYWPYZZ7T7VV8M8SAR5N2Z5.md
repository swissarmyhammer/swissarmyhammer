---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffb580
project: spatial-nav
title: Expose enumerateScopesInLayer() and layerFqOf() on SpatialFocusActions
---
## What

The Jump-To overlay needs to render a code label at the upper-left of every focusable scope visible right now in a *specific* focus layer (the layer the user was working in before the overlay opened).

**The infrastructure already exists.** The spatial-nav cutover (commits `0fd47753d`, `efe45028b`, `a8a990e06`) introduced `LayerScopeRegistry` (`kanban-app/ui/src/lib/layer-scope-registry-context.tsx`), the React-side authoritative scope-tracking system. Each `<FocusLayer>` instantiates its own `LayerScopeRegistry`; `<FocusScope>` mount effects call `registry.add(fq, entry)` (entry holds `ref`, `parentZone`, `navOverride`, `segment`, `lastKnownRect`). The registry exposes `entries()`, `has(fq)`, `size`, and `buildSnapshot(layerFq)`.

`SpatialFocusProvider` already maintains a `layerRegistriesRef: Map<FullyQualifiedMoniker, LayerScopeRegistry>` (populated via `actions.registerLayerRegistry`, used by the internal `buildSnapshotForFocused` helper).

So this task is **NOT** "build a parallel registration map." It is: surface two new methods on `SpatialFocusActions` that read from the existing `layerRegistriesRef`.

### Steps

1. In `kanban-app/ui/src/lib/spatial-focus-context.tsx`, add to the `SpatialFocusActions` interface:

   ```ts
   /**
    * Enumerate every currently-registered scope in `layerFq`'s registry.
    * Reads `getBoundingClientRect()` at call time for each entry's host
    * ref. Returns `[]` when the layer has no registry, or when no entries
    * have a live `ref.current`.
    */
   enumerateScopesInLayer: (
     layerFq: FullyQualifiedMoniker,
   ) => Array<{ fq: FullyQualifiedMoniker; rect: DOMRect }>;

   /**
    * Look up the layer FQM whose `LayerScopeRegistry` currently contains
    * `fq`. Returns `null` when no registry has the FQM (transient unmount
    * window, or unregistered FQM).
    */
   layerFqOf: (fq: FullyQualifiedMoniker) => FullyQualifiedMoniker | null;
   ```

2. Implementation in the provider:

   - `enumerateScopesInLayer(layerFq)`: look up `layerRegistriesRef.current.get(layerFq)`; if `undefined`, return `[]`. Otherwise walk `registry.entries()`, skip entries whose `ref.current` is null, and return `{ fq, rect: ref.current.getBoundingClientRect() }`. (No need to surface `kind` — only `<FocusScope>` registers in the registry; `<FocusZone>` does not, so every entry is a scope by construction.)
   - `layerFqOf(fq)`: walk `layerRegistriesRef.current.entries()`, return the first `layerFq` whose `registry.has(fq)` is true; return `null` if none match. Same pattern as the internal `buildSnapshotForFocused` helper.

3. Wire both into the `actions` bag returned by the provider so the JumpToOverlay (next task) can call them through `useSpatialFocusActions()`.

4. **Do NOT modify** `<FocusScope>`, `<FocusZone>`, `LayerScopeRegistry`, or `ScopeEntry`. The existing registration shape is sufficient. The earlier version of this task proposed extending the per-FQM claim map with `hostRef` / `layerFq` parameters — that's now redundant because `LayerScopeRegistry` already holds those.

### Why no `kind` field

The earlier version of this task included `kind: "zone" | "scope"` in the return shape. Drop it: `LayerScopeRegistry` only tracks `<FocusScope>` registrations (verified at `focus-scope.tsx` `useOptionalLayerScopeRegistry()` call site; `focus-zone.tsx` does not call it). Zones are containers, not Jump-To targets. Every enumerated entry is a scope.

## Acceptance Criteria

- [x] `SpatialFocusActions` exposes `enumerateScopesInLayer(layerFq) → { fq, rect }[]` and `layerFqOf(fq) → FullyQualifiedMoniker | null`.
- [x] `enumerateScopesInLayer` returns `[]` when the layer has no registry registered or when every entry has a null `ref.current`.
- [x] Returned rects come from a fresh `getBoundingClientRect()` call (no cache; same pattern as `LayerScopeRegistry.buildSnapshot`).
- [x] No new state — both methods read from the existing `layerRegistriesRef`.
- [x] No changes to `<FocusScope>`, `<FocusZone>`, `LayerScopeRegistry`, or `ScopeEntry`.

## Tests

- [x] New test `kanban-app/ui/src/lib/spatial-focus-context.enumerate.test.tsx`:
  - Mounts a tree with two `<FocusLayer>`s (window layer + a modal layer), each containing several `<FocusScope>`s.
  - Stubs `getBoundingClientRect` on each host (use the same helper the existing `*.spatial.test.tsx` files use).
  - `enumerateScopesInLayer(windowLayerFq)` returns only window-layer scopes with the stubbed rects.
  - `enumerateScopesInLayer(modalLayerFq)` returns only modal-layer scopes.
  - `enumerateScopesInLayer(unknownFq)` returns `[]`.
  - `layerFqOf` returns the correct layer FQM for a registered scope, `null` for an unregistered FQM.
  - Zero-rect / detached-element case (host `display: none` so `ref.current` is non-null but the layout is gone) — current behavior of `LayerScopeRegistry.buildSnapshot` is to include zero-rect entries; mirror that here so this method has the same contract. The Jump-To overlay (next task) is responsible for filtering zero-area rects when laying out pills.
- [x] Test command: `cd kanban-app/ui && pnpm test spatial-focus-context.enumerate` — passes.

## Workflow

- Use `/tdd` — write the enumerate test first; watch it fail (the actions don't exist yet); add the two methods to the actions bag and the provider; re-run.

## Notes from spatial-nav refactor

The cutover series (`a8a990e06`, `efe45028b`, `58fa22ee6`) replaced per-scope IPC registration with snapshot-driven decisions. `drillIn` / `drillOut` / `navigate` actions now thread a `NavSnapshot` built from `LayerScopeRegistry.buildSnapshot(layerFq)`. This task adds two more consumers of the same registry — Jump-To enumeration is conceptually a sibling of the nav decision snapshot, just consumed by React rather than by the kernel. #nav-jump