---
assignees:
- wballard
depends_on:
- 01KQW62XNHC1YP8ZKJGGFP0JZW
- 01KQW65Z689G7WWRYMBHX6MD7V
- 01KQW6880CFYR0A04RKBSJ79Q1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffaa80
project: spatial-nav
title: 'spatial-nav redesign step 6: spatial_navigate(snapshot) IPC variant + wire React nav handler'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Land the first user-facing snapshot path. React's nav handler builds a snapshot from `LayerScopeRegistry` and sends it to a new `spatial_navigate` IPC variant. The existing snapshot-less `spatial_navigate` continues to work; both paths must produce identical results.

## What to build

### Tauri command

`kanban-app/src/commands.rs`: extend `spatial_navigate` to accept an optional `snapshot: Option<NavSnapshot>` parameter (using serde's `#[serde(default)]` so existing callers without the field still work). When `snapshot` is `Some`, kernel runs pathfinding via the snapshot path (step 3). When `None`, it runs registry path (existing).

### Kernel call site

`SpatialState::navigate` (or whatever the adapter calls): branch on snapshot presence. Snapshot path uses the trait-based `geometric_pick` from step 3 against an `IndexedSnapshot::new(&snapshot)`. Registry path unchanged. After picking the target, the same path commits via `state.focus(target)` — which still uses `record_focus` registry-walk in this step. (Step 7 wires snapshot into `record_focus` at the focus call site.)

### React nav handler

`kanban-app/ui/src/lib/spatial-focus-context.tsx`'s `navigate` action:

```ts
const navigate: SpatialFocusActions["navigate"] = async (focusedFq, direction) => {
  const layerFq = enclosingLayerFqRef.current;
  const registry = layerRegistriesRef.current.get(layerFq);
  const snapshot = registry?.buildSnapshot(layerFq);
  await invoke("spatial_navigate", { focusedFq, direction, snapshot });
};
```

`buildSnapshot` reads `getBoundingClientRect()` on every entry's ref at this moment — fresh geometry at decision time.

### Result diagnostic

In dev builds, log when the snapshot-path result differs from the registry-path result for the same nav call. Run both internally; emit `tracing::warn!` on divergence with full context. This is the bedding-in instrumentation that proves correctness before cutover.

## Tests

- e2e: arrow-key nav from a kanban card with snapshot path enabled — produces the same target as today (registry path).
- Unit (Rust): `spatial_navigate(focused_fq, direction, Some(snapshot))` returns same result as `spatial_navigate(focused_fq, direction, None)` for matching scope sets.
- Integration (React): `actions.navigate` emits an IPC with a populated snapshot field. Snapshot contains every live scope under the active layer with correct `parent_zone` and `nav_override` values.
- Regression: existing nav tests that don't supply a snapshot still pass against registry path.

## Out of scope

- `spatial_focus` snapshot variant (step 7)
- `spatial_focus_lost` (step 8)
- Removing the registry path at the call boundary (step 11)

## Acceptance criteria

- React arrow-key nav lands the same target via either path
- Dev build emits zero divergence warnings in normal kanban use
- `cargo test` and `pnpm -C kanban-app/ui test` green

## Files

- `kanban-app/src/commands.rs` — `spatial_navigate` accepts optional snapshot
- `swissarmyhammer-focus/src/state.rs` — `SpatialState::navigate` branches on snapshot
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `actions.navigate` builds snapshot
- `kanban-app/ui/src/types/spatial.ts` — `NavSnapshot` / `SnapshotScope` TS types finalized to match Rust serde shape #stateless-nav