---
assignees:
- wballard
position_column: todo
position_ordinal: dd80
project: spatial-nav
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

Stop replicating. The Rust kernel becomes **stateless with respect to scope geometry and structure** — it holds only the focus state machine (current focus, layer stack, focus-restoration history) and the pathfinding + fallback algorithms. There is no scope registry in Rust. Two changes accomplish this:

### 1. Move the scope registry into React.

Each `<FocusLayer>` provides a `LayerScopeRegistry` via context. The registry is a `Map<FullyQualifiedMoniker, ScopeEntry>` held in a ref on the layer, where:

```ts
interface ScopeEntry {
  ref: React.RefObject<HTMLElement>;     // for getBoundingClientRect at decision time
  parentZone: FullyQualifiedMoniker | null;
  navOverride?: FocusOverrides;
  segment: SegmentMoniker;
}
```

Each `<FocusScope>` registers itself via `useEffect(() => registry.add(fq, entry); return () => registry.delete(fq), [fq, ...])`. This is the same pattern `spatial-focus-context.tsx:355-369` already uses for focus claims — extended to track *what scopes exist*, not just *who has focus*.

React's deterministic effect cleanup guarantees the unmount path. No async IPC, no `.catch` to swallow failures, no Promise to drop. If unmount runs, the entry is gone synchronously. There is no kernel "drop" path because there is no kernel state to drop.

### 2. Build the kernel's view of the world per-decision.

Every kernel call that touches focus carries a fresh snapshot of the active layer. The kernel never reads scope state out-of-band, because it doesn't have any.

When the user navigates (or clicks, or the focused scope unmounts), React's handler:

1. Walks the active layer's registry
2. For each `ScopeEntry`, reads `entry.ref.current?.getBoundingClientRect()` and builds `(fq, rect, parent_zone, nav_override)` tuples
3. Sends the appropriate IPC with the snapshot inline

The kernel runs pathfinding (or fallback resolution) on the snapshot, returns the result, React commits. If a DOM lookup fails (rare race during animation), React calls again with a fresh snapshot — the bad FQ won't appear because the registry already removed it.

Virtualization caps the snapshot at viewport-bounded size (tens to low hundreds of scopes). `getBoundingClientRect` over that set is sub-millisecond. Decisions are human-paced; the snapshot cost is invisible.

## What gets deleted

### Rust side (`swissarmyhammer-focus`)

- `SpatialRegistry::scopes: HashMap<FQM, FocusScope>` and all methods on it
- `register_scope`, `unregister_scope`, `update_rect` (no replica → no register/unregister/update path at all; not even a cascade variant)
- `check_overlap_warning` (moves to JS; see below)
- `validated_layers` cache (no scopes to validate)
- `overlap_warn_partner` map
- `record_focus`'s reliance on `registry.scopes` (rewritten to walk the snapshot)

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

- **`layers: Map<FQM, FocusLayer>`** — modal stack. Slow-changing (push/pop only). `FocusLayer::last_focused` continues to live here, set on every focus event, restored on layer pop. Unchanged from today.
- **`focus_by_window: Map<WindowLabel, FQM>`** — current focus per window. Unchanged from today.
- **`last_focused_by_fq: Map<FQM, FQM>`** — *new top-level field*, replacing the per-scope `last_focused: Option<FQM>` slot that today lives on `FocusScope`. Keyed by ancestor FQM, value is the last descendant FQM that was focused under it. Populated by `record_focus` (see below). Never freed proactively — entries are O(visited-ancestors) per session, tiny, and dormant when their key isn't in any current snapshot. Optional later: bound by an LRU cap if it ever matters.
- **Pathfinding** (`navigate` direction logic, beam search, override resolution) — adapted to take a snapshot argument instead of consulting `registry.scopes`.
- **`resolve_fallback`** — the rule cascade that picks a new focus when the current one is lost (sibling-in-zone → parent-zone last_focused → parent-zone nearest → parent-layer last_focused → parent-layer nearest → NoFocus). Today walks `registry.scopes` and `registry.layers`. Adapted to walk the snapshot's `parent_zone` chain plus the surviving `layers` + `last_focused_by_fq`.
- **`record_focus`** — on a successful focus mutation, walks the focused FQ's ancestor chain and writes each ancestor's `last_focused` slot. Today walks `registry.scopes[fq].parent_zone` recursively. Rewritten to walk the snapshot's `parent_zone` chain for the focused FQ, then the layer chain via `layers[fq].parent`. Writes go to `last_focused_by_fq` (for scope ancestors) and `layers[*].last_focused` (for layer ancestors).

## New / modified IPC surface

Every focus-mutating IPC carries a snapshot. The kernel never resolves ancestry from internal state — it gets it from the call.

```
spatial_focus(fq, snapshot)
  // explicit focus: click, programmatic, or React confirming a kernel-suggested target.
  // Kernel:
  //   1. validates `fq` is in snapshot.scopes
  //   2. writes focus_by_window[window] = fq
  //   3. record_focus(fq, snapshot) — walks snapshot.parent_zone chain + layer chain,
  //      writes last_focused_by_fq and layer.last_focused for every ancestor
  //   4. returns FocusChangedEvent

spatial_navigate(focused_fq, direction, snapshot)
  // arrow keys.
  // Kernel runs pathfinding on snapshot, picks target, then commits exactly as
  // spatial_focus does (writes focus_by_window, runs record_focus on snapshot).
  // Returns FocusChangedEvent.

spatial_focus_lost(focused_fq, snapshot)
  // NEW. React calls this when its registry's delete handler observes that the
  // FQM being removed equals `currentFocus`. Kernel runs resolve_fallback
  // against the snapshot, commits the new focus, returns FocusChangedEvent.
  // The lost FQM does NOT appear in snapshot.scopes (already removed by React).
  // resolve_fallback walks parent_zone / layer.parent looking for a live target.

spatial_clear_focus()
  // Unchanged. Pure focus-state mutation, no ancestry needed.

spatial_push_layer(...)
spatial_pop_layer(...)
  // Unchanged. Layer state is push-only and self-contained; layer.last_focused
  // is restored on pop and emitted as the FocusChangedEvent's next_fq.
  // React then calls spatial_focus(restored_fq, snapshot) to commit & ancestry-walk.
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

Pathfinding and fallback both treat the snapshot as a flat `Vec<SnapshotScope>` plus a precomputed `HashMap<FQ, &SnapshotScope>` built once per call for parent_zone walks. Both algorithms become pure functions of `(snapshot, layers, last_focused_by_fq, focused_fq, ...)`.

## Focus-lost handling — the UX-critical case

Today: when a focused scope is unregistered, `state.handle_unregister` runs `resolve_fallback` and either lands focus on a sibling, or on the parent zone's `last_focused`, or on the parent layer's `last_focused`, or clears focus. This is what keeps the user from getting stranded when their focused element disappears (e.g., the focused chip's task is deleted, or filter changes hide the focused row).

Redesign: same behavior, different transport. React's `LayerScopeRegistry` `delete(fq)` handler does:

```ts
function delete(fq) {
  registry.delete(fq);
  if (currentFocusFq === fq) {
    const snapshot = buildSnapshot(layerFq);
    invoke("spatial_focus_lost", { focusedFq: fq, snapshot });
  }
}
```

The kernel runs the same `resolve_fallback` rule cascade — just walking the snapshot's `parent_zone` instead of `registry.scopes[*].parent_zone`. The fallback variants (`FallbackSiblingInZone`, `FallbackParentZoneLastFocused`, etc.) are unchanged.

This is why the snapshot must include `parent_zone` for every entry: pathfinding doesn't strictly need it, but `resolve_fallback` walks ancestor chains.

## navOverride lifecycle change

Today: `navOverride` is read at register time, snapshot into `registry.scopes[fq].nav_override`, mid-life changes ignored (see comment in `focus-scope.tsx:336-343`).

Redesign: `navOverride` is read at decision time from the React-side `ScopeEntry` and shipped in the snapshot. Mid-life changes take effect on the next nav. This is a behavior *improvement* — the previous "ignored" semantics existed only because mid-life IPC was costly. With snapshot-on-decision, the cost is zero and the behavior matches what callers naively expect.

## What replaces the overlap warning

`check_overlap_warning` becomes a dev-mode JS assertion that runs against the layer registry on registration:

```ts
if (DEV) registry.add hooks: warn if two entries share rect after first paint
```

Runs in JS, has direct refs to both elements, can include the React component-stack in the warning. False positives during drag-drop disappear because the warning runs at registration time (or at nav time on settled state), not on every animation frame.

## Migration plan

This is a sizable refactor. Land in stages:

1. **Add the React-side `LayerScopeRegistry` alongside the existing kernel sync.** Both sources of truth, kernel still authoritative. Gives a place to instrument.
2. **Add `spatial_navigate`, `spatial_focus`, `spatial_focus_lost` variants that accept a snapshot.** Kernel uses snapshot if provided, falls back to internal state otherwise. Switch React to send snapshots on every focus-mutating IPC.
3. **Verify nav results, focus results, and fallback results match between snapshot path and replica path** in dev mode. Log mismatches; fix bugs.
4. **Cut over: snapshot path becomes the only path.** Delete `spatial_register_scope` / `spatial_unregister_scope` / `spatial_update_rect` from kernel + frontend. Delete `SpatialRegistry::scopes` and the per-scope `last_focused` field; introduce `last_focused_by_fq`.
5. **Move overlap-warning to JS dev-mode.** Delete the Rust path.

Tests at each stage:
- Stage 1: registry contents match kernel `scopes` map in unit/integration tests
- Stage 2: snapshot-path nav / focus / focus-lost all return the same result as replica-path for the existing test cases
- Stage 3: the dual-source comparison runs cleanly under drag-drop, layer push/pop, and bulk filter changes
- Stage 4: regression suite for the deleted commands stays green via mocks; remaining e2e nav and fallback tests pass against snapshot path
- Stage 5: the original symptom (overlap warning during drag) does not fire

## Files (preliminary)

- `swissarmyhammer-focus/src/registry.rs` — major shrink; loses `scopes`, gains `last_focused_by_fq`
- `swissarmyhammer-focus/src/state.rs` — `focus`, `handle_unregister` → `focus_lost`, `resolve_fallback` adapted to take a snapshot argument; `record_focus` becomes a snapshot walk
- `swissarmyhammer-focus/src/snapshot.rs` (new) — `NavSnapshot`, `SnapshotScope`, snapshot-walk helpers (`parent_zone_chain_in_snapshot`, etc.)
- `kanban-app/src/commands.rs` — IPC surface change (`spatial_focus_lost`; snapshot args on `focus`, `navigate`)
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `LayerScopeRegistry`, snapshot builder, focus-lost detection
- `kanban-app/ui/src/components/focus-scope.tsx` — drop ResizeObserver and `spatial_register_scope`; add registry registration
- `kanban-app/ui/src/components/focus-layer.tsx` — provide `LayerScopeRegistryContext`
- `kanban-app/ui/src/lib/rect-validation.ts` — slim down to per-snapshot validator
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — delete

## Why this is the right answer

Drift is a property of replication. The kernel and React are currently two replicas of the same logical tree, kept in sync via async IPC. *Any* replication strategy under async IPC has failure modes — cascade leaks, ordering races, silent drops. Reconciliation/sync protocols add complexity to mitigate failure modes that wouldn't exist if there were nothing to replicate.

The kernel doesn't fundamentally need to know what scopes exist between decisions. It needs to know *at the moment of a decision*. So push that knowledge across exactly when it's needed. State that doesn't exist can't be stale.

Plus: virtualization caps snapshot size, so the cost argument against this design (which would have been real with thousands of off-screen scopes) doesn't apply here. The architecture matches the runtime characteristics. #01KQTC1VNQM9KC90S65P7QX9N1