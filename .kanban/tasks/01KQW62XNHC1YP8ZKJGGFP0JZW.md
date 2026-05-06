---
assignees:
- wballard
position_column: review
position_ordinal: '8480'
project: spatial-nav
title: 'spatial-nav redesign step 1: add LayerScopeRegistry context (additive, no behavior change)'
---
## Parent

  Implementation step for the architectural redesign in **01KQTC1VNQM9KC90S65P7QX9N1**. Read that card first for the rationale.

## Goal

Stand up a React-side `LayerScopeRegistry` populated by every `<FocusScope>` mount/unmount, **alongside** the existing kernel sync. Both sources of truth coexist for now. No user-visible behavior change. This unlocks all subsequent steps that need to build snapshots from React state.

## What to build

### `LayerScopeRegistryContext`

New context in `kanban-app/ui/src/lib/spatial-focus-context.tsx` (or a new `layer-scope-registry-context.tsx` if cleaner):

```ts
interface ScopeEntry {
  ref: React.RefObject<HTMLElement>;
  parentZone: FullyQualifiedMoniker | null;
  navOverride?: FocusOverrides;
  segment: SegmentMoniker;
}

interface LayerScopeRegistry {
  add(fq: FullyQualifiedMoniker, entry: ScopeEntry): void;
  delete(fq: FullyQualifiedMoniker): void;
  has(fq: FullyQualifiedMoniker): boolean;
  entries(): IterableIterator<[FullyQualifiedMoniker, ScopeEntry]>;
  // for snapshot building in later steps
  buildSnapshot(layerFq: FullyQualifiedMoniker): NavSnapshot;
}
```

Backing store: `Map<FQM, ScopeEntry>` in a ref on the layer. Same pattern as `spatial-focus-context.tsx:355-369`'s claim registry.

### `<FocusLayer>` provides the registry

In `kanban-app/ui/src/components/focus-layer.tsx`, wrap children in `<LayerScopeRegistryContext.Provider value={registry}>`. Each layer has its own registry — registries do NOT cross modal boundaries.

### `<FocusScope>` registers itself

In `kanban-app/ui/src/components/focus-scope.tsx`, add a useEffect that registers/unregisters in the layer registry. Run **alongside** the existing `registerSpatialScope` / `unregisterScope` IPC calls in the existing useEffect (lines 345-403) — do NOT remove the existing registration. Both paths live for now.

```ts
const layerRegistry = useContext(LayerScopeRegistryContext);
useEffect(() => {
  if (!layerRegistry) return;
  layerRegistry.add(fq, { ref, parentZone, navOverride, segment });
  return () => layerRegistry.delete(fq);
}, [fq, segment, parentZone, navOverride, layerRegistry]);
```

`navOverride` is read live (not snapshotted into a ref) so mid-life changes are visible in the registry — the redesign explicitly improves this behavior.

### Snapshot builder stub

`buildSnapshot(layerFq)` walks `entries()` and, for each, reads `entry.ref.current?.getBoundingClientRect()`. Returns `NavSnapshot { layer_fq, scopes: SnapshotScope[] }`. The Rust-side `NavSnapshot` types come in step 2; for now, build a TypeScript-side `NavSnapshot` interface that matches the planned shape:

```ts
interface NavSnapshot {
  layer_fq: FullyQualifiedMoniker;
  scopes: SnapshotScope[];
}
interface SnapshotScope {
  fq: FullyQualifiedMoniker;
  rect: PixelRect;
  parent_zone: FullyQualifiedMoniker | null;
  nav_override: FocusOverrides;
}
```

Skip an entry if `entry.ref.current === null` (transient unmount window).

## Tests

- New unit test: mount a `<FocusLayer>` with several nested `<FocusScope>` children, assert the layer's registry contains all of them with correct `parentZone` chains.
- Unmount a subset, assert registry shrinks correctly.
- Re-render with changed `parentZone` (e.g., reparent a scope) and assert the registry entry updates.
- **Parity test**: at any moment, the React layer registry contents (FQ set) match the kernel's `registry.scopes` FQ set for that layer. Mount/unmount sequences should keep them in lockstep. This is the diagnostic that proves the dual-source model is working before we cut over.

## Out of scope for this step

- Changing kernel behavior (steps 2–5)
- Building or shipping snapshots over IPC (steps 6–8)
- Removing the existing kernel sync (steps 10–12)

## Acceptance criteria

- LayerScopeRegistry exists, populated on mount, drained on unmount.
- Existing kernel sync paths unchanged; nav still works exactly as before.
- New parity test passes for a representative kanban-board scene.
- `pnpm -C kanban-app/ui test` green.

## Files

- `kanban-app/ui/src/lib/spatial-focus-context.tsx` (or new context file)
- `kanban-app/ui/src/components/focus-layer.tsx`
- `kanban-app/ui/src/components/focus-scope.tsx`
- New test file: `kanban-app/ui/src/lib/layer-scope-registry.test.tsx` #stateless-nav