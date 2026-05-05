---
assignees:
- wballard
position_column: todo
position_ordinal: cf8180
title: 'spatial-nav: redesign — React-side scope registry, kernel as stateless FSM over per-nav snapshots'
---
## Premise

The current spatial-nav design replicates React's scope tree into a Rust kernel via `spatial_register_scope` / `spatial_unregister_scope` / `spatial_update_rect`. The replica drifts under:

- Async IPC failures (silent `.catch(console.error)` in `focus-scope.tsx:389-394`)
- Async ordering races (concurrent `update_rect` from sibling reflow)
- Missing parent-cascade on unmount (kernel removes single FQ, not subtree)
- Drag-drop and animation (rects shift without scopes mounting/unmounting; ResizeObserver only fires on size, not position)
- Sibling-reflow staleness (a card moves because a sibling reordered — its size didn't change, ResizeObserver doesn't fire, kernel rect stays stale)

Symptom: `check_overlap_warning` logs `two entries share (x, y)` for two different live tasks' chips — one rect is correct, the other is a kernel ghost (cascade leak) or a sibling-reflow stale rect.

The deeper problem is replication itself. Any incremental fix (cascade-drop, reconciliation, fingerprinting, generation tags) is a band-aid for a class of bugs that only exists because two systems share mutable state over async IPC. Remove the shared state and the entire bug class disappears.

## The redesign

Stop replicating. The Rust kernel becomes **stateless with respect to scope geometry and structure** — it holds only the focus state machine (current focus, layer stack, focus-restoration history) and the pathfinding algorithm. There is no scope registry in Rust. Two changes accomplish this:

### 1. Move the scope registry into React.

Each `<FocusLayer>` provides a `LayerScopeRegistry` via context. The registry is a `Map<FullyQualifiedMoniker, ScopeEntry>` held in a ref on the layer, where:

```ts
interface ScopeEntry {
  ref: React.RefObject<HTMLElement>;     // for getBoundingClientRect at nav time
  parentZone: FullyQualifiedMoniker | null;
  navOverride?: FocusOverrides;
  segment: SegmentMoniker;
}
```

Each `<FocusScope>` registers itself via `useEffect(() => registry.add(fq, entry); return () => registry.delete(fq), [fq, ...])`. This is the same pattern `spatial-focus-context.tsx:355-369` already uses for focus claims — extended to track *what scopes exist*, not just *who has focus*.

React's deterministic effect cleanup guarantees the unmount path. No async IPC, no `.catch` to swallow failures, no Promise to drop. If unmount runs, the entry is gone synchronously. There is no kernel "drop" path because there is no kernel state to drop.

### 2. Build the kernel's view of the world per-nav.

When the user navigates, React's nav handler:

1. Walks the active layer's registry
2. For each `ScopeEntry`, reads `entry.ref.current?.getBoundingClientRect()` and builds `(fq, rect, parent_zone, nav_override)` tuples
3. Sends `spatial_navigate(focused_fq, direction, snapshot)` with the snapshot inline

The kernel runs pathfinding on the snapshot, returns `next_fq`. React focuses that FQ. If the DOM lookup fails (rare race during animation), React calls `spatial_navigate` again with the same focused FQ and a fresh snapshot — the bad FQ won't appear because the registry already removed it.

Virtualization caps the snapshot at viewport-bounded size (tens to low hundreds of scopes). `getBoundingClientRect` over that set is sub-millisecond. Nav is human-paced; the snapshot cost is invisible.

## What gets deleted

### Rust side (`swissarmyhammer-focus`)

- `SpatialRegistry::scopes: HashMap<FQM, FocusScope>` and all methods on it
- `register_scope`, `unregister_scope`, `update_rect` (no replica → no register/unregister/update path at all; not even a cascade variant)
- `check_overlap_warning` (moves to JS; see below)
- `validated_layers` cache (no scopes to validate)
- `overlap_warn_partner` map
- `record_focus`'s scope-ancestor walk (rebuilt per-nav from snapshot)

### Tauri commands (`kanban-app/src/commands.rs`)

- `spatial_register_scope`
- `spatial_unregister_scope`
- `spatial_update_rect`

### Frontend

- All `registerSpatialScope` / `unregisterScope` / `updateRect` calls in `spatial-focus-context.tsx`
- The ResizeObserver in `focus-scope.tsx:371-387`
- `useTrackRectOnAncestorScroll` (no continuous rect sync needed)
- `register_scope` / `register_zone` / `update_rect` ops in `rect-validation.ts` (becomes a per-snapshot validator)

## What survives in Rust

The genuinely Rust-shaped state machine:

- `layers: Map<FQM, FocusLayer>` (modal stack — slow-changing, push/pop)
- `focus_by_window: Map<WindowLabel, FQM>`
- `last_focused: Map<FQM, FQM>` (focus restoration history; updated on focus events; entries dormant when their FQ is absent from the current snapshot)
- Pathfinding algorithm (`navigate` direction logic, beam search, override resolution)

## New / modified IPC surface

```
spatial_focus(fq)                                  // click / explicit focus
spatial_navigate(focused_fq, direction, snapshot)  // arrow keys, with payload
spatial_clear_focus()
spatial_push_layer(...)                            // unchanged
spatial_pop_layer(...)                             // unchanged
```

`Snapshot` payload shape (Rust types mirror the JS):

```rust
struct NavSnapshot {
    layer_fq: FullyQualifiedMoniker,
    scopes: Vec<SnapshotScope>,
}
struct SnapshotScope {
    fq: FullyQualifiedMoniker,
    rect: PixelRect,
    parent_zone: Option<FullyQualifiedMoniker>,
    nav_override: FocusOverrides,
}
```

## What replaces the overlap warning

`check_overlap_warning` becomes a dev-mode JS assertion that runs against the layer registry on registration:

```ts
if (DEV) registry.add hooks: warn if two entries share rect after first paint
```

Runs in JS, has direct refs to both elements, can include the React component-stack in the warning. False positives during drag-drop disappear because the warning runs at registration time (or at nav time on settled state), not on every animation frame.

## Migration plan

This is a sizable refactor. Land in stages:

1. **Add the React-side registry alongside the existing kernel sync.** Both sources of truth, kernel still authoritative for nav. Gives us a place to instrument and compare.
2. **Add `spatial_navigate(focused_fq, direction, snapshot)` accepting an optional snapshot.** Kernel uses snapshot if provided, falls back to internal state. Switch React to send snapshots.
3. **Verify nav results match between snapshot path and replica path** in dev mode. Log mismatches; fix bugs.
4. **Cut over: snapshot path becomes the only path.** Delete `spatial_register_scope` / `spatial_unregister_scope` / `spatial_update_rect` from kernel + frontend. Delete `SpatialRegistry::scopes`.
5. **Move overlap-warning to JS dev-mode.** Delete the Rust path.

Tests at each stage:
- Stage 1: registry contents match kernel contents in unit/integration tests
- Stage 2: snapshot-path nav returns the same target as replica-path nav for the existing test cases
- Stage 4: regression suite for the deleted commands stays green via mocks; remaining e2e nav tests pass against snapshot path
- Stage 5: the original symptom (overlap warning during drag) does not fire

## Files (preliminary)

- `swissarmyhammer-focus/src/registry.rs` — major shrink
- `swissarmyhammer-focus/src/state.rs` — pathfinding adapted to take a snapshot argument
- `swissarmyhammer-focus/src/snapshot.rs` (new) — snapshot types + helpers
- `kanban-app/src/commands.rs` — IPC surface change
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — registry + nav handler
- `kanban-app/ui/src/components/focus-scope.tsx` — drop ResizeObserver, add registry registration
- `kanban-app/ui/src/components/focus-layer.tsx` — provide LayerScopeRegistry context
- `kanban-app/ui/src/lib/rect-validation.ts` — slim down to per-snapshot validator
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — delete

## Why this is the right answer

Drift is a property of replication. The kernel and React are currently two replicas of the same logical tree, kept in sync via async IPC. *Any* replication strategy under async IPC has failure modes — cascade leaks, ordering races, silent drops. Reconciliation/sync protocols add complexity to mitigate failure modes that wouldn't exist if there were nothing to replicate.

The kernel doesn't fundamentally need to know what scopes exist between navigations. It needs to know *at the moment of a nav decision*. So push that knowledge across exactly when it's needed. State that doesn't exist can't be stale.

Plus: virtualization caps snapshot size, so the cost argument against this design (which would have been real with thousands of off-screen scopes) doesn't apply here. The architecture matches the runtime characteristics.