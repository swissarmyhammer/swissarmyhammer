---
assignees:
- wballard
depends_on:
- 01KQW62XNHC1YP8ZKJGGFP0JZW
- 01KQW65Z689G7WWRYMBHX6MD7V
- 01KQW6880CFYR0A04RKBSJ79Q1
position_column: todo
position_ordinal: d680
project: spatial-nav
title: 'spatial-nav redesign step 7: spatial_focus(snapshot) IPC variant + wire click and programmatic focus'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Carry snapshot through the explicit-focus path (click, programmatic focus, focus restoration after layer pop). Mirror image of step 6, but for `spatial_focus` instead of `spatial_navigate`.

## What to build

### Tauri command

`kanban-app/src/commands.rs`: extend `spatial_focus` to accept optional `snapshot: Option<NavSnapshot>`. When `Some`, kernel commits focus and runs `record_focus(fq, Some(snapshot))` from step 5. When `None`, runs registry-walk (existing).

### Kernel call site

`SpatialState::focus`: branch on snapshot. Snapshot path: validate `fq` is in `snapshot.scopes`, write `focus_by_window`, run `record_focus(fq, Some(snapshot))`. Registry path: existing behavior.

### React click handler

`focus-scope.tsx::handleClick` calls `focus(fq)` from `useSpatialFocusActions`. Update the action to build a snapshot from the current `LayerScopeRegistry` and pass it through:

```ts
const focus: SpatialFocusActions["focus"] = async (fq) => {
  const layerFq = enclosingLayerFqRef.current;
  const registry = layerRegistriesRef.current.get(layerFq);
  const snapshot = registry?.buildSnapshot(layerFq);
  await invoke("spatial_focus", { fq, snapshot });
};
```

### Layer-pop restoration

`spatial_pop_layer` returns `FocusChangedEvent` with `next_fq = layer.last_focused`. Today the kernel calls `state.focus(next_fq)` internally to commit. After this step, that internal commit needs a snapshot — but the kernel doesn't have one. Two options:

(a) Kernel returns `next_fq` to React without committing; React then calls `spatial_focus(next_fq, snapshot)` to commit.
(b) Kernel keeps an internal "snapshot-less" commit path for layer-pop only, since layer-pop's restoration target is by definition known to the kernel.

Recommended: (a). Cleaner separation — the kernel never commits without a snapshot, all ancestry walks are snapshot-driven. Layer-pop becomes a request/response: kernel says "focus should restore to X," React confirms with `spatial_focus(X, snapshot)`. Update `spatial-focus-context.tsx`'s layer-pop handler to do this round-trip.

### Divergence diagnostic

Same pattern as step 6: dev-mode dual-run, log `tracing::warn!` on divergence between snapshot path and registry path.

## Tests

- Click on a card calls `spatial_focus` with a populated snapshot. Result identical to today.
- Programmatic focus (e.g., from a command) passes snapshot.
- Layer push → push child layer → pop child layer: focus restores to the last_focused via the round-trip. State after restoration matches today.
- Snapshot-path `state.focus` produces identical `last_focused_by_fq` writes as registry-path for matching scope sets.

## Out of scope

- `spatial_focus_lost` (step 8)
- Removing the registry path (step 12)

## Acceptance criteria

- Click-to-focus and programmatic focus both go through the snapshot path
- Layer-pop restoration works via the round-trip pattern
- Dev-mode divergence warnings stay at zero
- All tests green

## Files

- `kanban-app/src/commands.rs` — `spatial_focus` accepts optional snapshot
- `swissarmyhammer-focus/src/state.rs` — `state.focus` branches on snapshot
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `actions.focus` builds snapshot; layer-pop event handler does round-trip #01KQTC1VNQM9KC90S65P7QX9N1