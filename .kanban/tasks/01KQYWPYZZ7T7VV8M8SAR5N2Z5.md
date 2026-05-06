---
assignees:
- claude-code
position_column: todo
position_ordinal: e080
project: spatial-nav
title: Add enumerateScopesInLayer() and layerFqOf() to SpatialFocusProvider
---
## What

The Jump-To overlay needs to render a code label at the upper-left of every focusable scope visible right now. To do that it needs to enumerate scopes in a *specific* focus layer (the one the user was working in before the overlay opened), with their fully-qualified moniker (FQM) and on-screen rect.

Today `SpatialFocusProvider` (`kanban-app/ui/src/lib/spatial-focus-context.tsx`) maintains an in-memory `Map<FullyQualifiedMoniker, FocusClaimListener>` for focus-claim notification, but it does NOT track host element refs (needed for rect lookup) or layer membership (needed for filtering). Add that.

Implementation:

1. In `kanban-app/ui/src/lib/spatial-focus-context.tsx`:
   - Extend the registration interface used by `<FocusScope>` (today `useFocusClaim(fq, listener)`) with two new optional parameters: `hostRef: RefObject<HTMLElement | null>` and `layerFq: FullyQualifiedMoniker` (the FQM of the enclosing `<FocusLayer>`). Store all four pieces in the registry map: `Map<FullyQualifiedMoniker, { listener, hostRef, kind, layerFq }>`.
   - The layer FQM is already available to descendants via `LayerFqContext` (see `kanban-app/ui/src/components/layer-fq-context.tsx` referenced from `focus-layer.tsx:68`). `<FocusScope>` can read it with `useContext(LayerFqContext)` and pass it to the registration call.
   - Expose a new actions method on `SpatialFocusActions`: `enumerateScopesInLayer(layerFq: FullyQualifiedMoniker): Array<{ fq: FullyQualifiedMoniker, rect: DOMRect, kind: "zone" | "scope" }>`. Takes the layer FQM the caller wants to enumerate; does NOT auto-pick "topmost". The caller (Jump-To overlay) picks the layer it's covering.
   - Implementation: for each entry in the registry where `entry.layerFq === layerFq`, if `entry.hostRef.current` is non-null AND its rect has non-zero area, include it with a fresh `getBoundingClientRect()` call; otherwise skip.
   - Also expose a helper: `actions.layerFqOf(fq: FullyQualifiedMoniker): FullyQualifiedMoniker | null` — returns the layer FQM a given scope's registration is in. The Jump-To overlay needs this to derive "the layer the user was focused in before the overlay opened" from the prior focused FQM.

2. In `kanban-app/ui/src/components/focus-scope.tsx`:
   - Pass `hostRef` (the existing ref it already holds for `getBoundingClientRect`) AND `layerFq` (read from `LayerFqContext`) through to the registration call.
   - Same change in `kanban-app/ui/src/components/focus-zone.tsx` (FocusZone registers separately — verify by reading the file).

3. Rect provenance: read `hostRef.current.getBoundingClientRect()` at enumeration time — one-shot read, the user is paused, no stale-rect concern. Do NOT cache rects.

## Acceptance Criteria

- [ ] `SpatialFocusActions` exposes `enumerateScopesInLayer(layerFq) → { fq, rect, kind }[]` and `layerFqOf(fq) → layerFq | null`.
- [ ] Returned list contains only scopes whose registration `layerFq` matches the argument AND whose host element has a non-zero rect.
- [ ] Returned rects come from a fresh `getBoundingClientRect()` call (no stale cache).
- [ ] `<FocusScope>` and `<FocusZone>` register their host ref AND their layer FQM with the provider; no other call sites need to change.

## Tests

- [ ] New test `kanban-app/ui/src/lib/spatial-focus-context.enumerate.test.tsx`:
  - Mounts a tree with two `<FocusLayer>`s (a window layer and a modal/inspector layer), each containing several `<FocusScope>`s.
  - Stubs `getBoundingClientRect` on each host (use the same helper the existing `*.spatial.test.tsx` files use).
  - Calls `enumerateScopesInLayer(windowLayerFq)` and asserts the returned list contains only the window-layer scopes with the correct rects.
  - Calls `enumerateScopesInLayer(modalLayerFq)` and asserts the returned list contains only the modal-layer scopes.
  - Tests `layerFqOf` returns the correct layer FQM for a registered scope, and `null` for an unregistered FQM.
  - Tests the empty case (no scopes registered in a layer) → returns `[]`.
  - Tests the zero-rect case (scope registered but element is `display: none` or detached) → that scope is excluded.
- [ ] Test command: `cd kanban-app/ui && pnpm test spatial-focus-context.enumerate` — passes.

## Workflow

- Use `/tdd` — write the enumerate test first; watch it fail (the actions methods don't exist); add the API and update FocusScope/FocusZone; re-run. #nav-jump