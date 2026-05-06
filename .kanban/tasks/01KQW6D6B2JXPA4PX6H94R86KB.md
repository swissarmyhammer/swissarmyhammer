---
assignees:
- wballard
depends_on:
- 01KQW62XNHC1YP8ZKJGGFP0JZW
- 01KQW675YPMAW9AGV4P80Y64V3
- 01KQW6880CFYR0A04RKBSJ79Q1
position_column: todo
position_ordinal: d780
project: spatial-nav
title: 'spatial-nav redesign step 8: spatial_focus_lost IPC + wire focused-scope unmount detection'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Replace the kernel-driven `state.handle_unregister` flow with a React-driven `spatial_focus_lost` IPC. When the focused scope unmounts on the React side, React detects it, builds a snapshot (which already excludes the lost FQ since the registry just deleted it), and asks the kernel to compute fallback focus.

## What to build

### New Tauri command

`kanban-app/src/commands.rs`:

```rust
#[tauri::command]
pub async fn spatial_focus_lost(
    focused_fq: FullyQualifiedMoniker,
    lost_parent_zone: Option<FullyQualifiedMoniker>,
    lost_layer_fq: FullyQualifiedMoniker,
    snapshot: NavSnapshot,
    // ... state, window
) -> Result<Option<FocusChangedEvent>, String>
```

Why three "lost" fields: the lost FQ is no longer in the registry OR the snapshot, but `resolve_fallback` needs its `parent_zone` and `layer_fq` to start the walk. React knows these because it had the entry in the registry until the moment of unmount.

### Kernel implementation

In `state.rs`:

```rust
pub fn focus_lost(
    &mut self,
    registry: &mut SpatialRegistry,
    snapshot: &NavSnapshot,
    lost_fq: &FullyQualifiedMoniker,
    lost_parent_zone: Option<&FullyQualifiedMoniker>,
    lost_layer_fq: &FullyQualifiedMoniker,
) -> Option<FocusChangedEvent>
```

Body: identical decision tree to existing `handle_unregister`, but reads metadata from `snapshot` + the explicit lost_* fields rather than from `registry.scopes`. Calls `resolve_fallback` with snapshot path (step 4), commits the result, runs `record_focus` with snapshot path (step 5), returns event.

### React detection

`LayerScopeRegistry.delete(fq)` (the registry from step 1) gets a hook:

```ts
function delete(fq: FullyQualifiedMoniker) {
  const entry = entries.get(fq);
  if (!entry) return;
  entries.delete(fq);

  if (currentFocusFqRef.current === fq) {
    // Build snapshot AFTER deletion — the lost FQ is correctly absent.
    const snapshot = buildSnapshot(layerFq);
    invoke("spatial_focus_lost", {
      focusedFq: fq,
      lostParentZone: entry.parentZone,
      lostLayerFq: layerFq,
      snapshot,
    }).catch((err) => console.error("[spatial_focus_lost] failed", err));
  }
}
```

`currentFocusFqRef` is the React-side mirror of `focus_by_window` for this window — the FocusContext already exposes it.

### Coexistence with existing handle_unregister

In this transitional step, the existing `spatial_unregister_scope` IPC (which triggers `state.handle_unregister`) still runs alongside `spatial_focus_lost`. They produce the same FocusChangedEvent (one of them might be a no-op depending on order). Order is:

1. React's `LayerScopeRegistry.delete(fq)` fires → if focused, calls `spatial_focus_lost` with snapshot (excluding fq)
2. React's existing `<FocusScope>` cleanup effect fires → calls `spatial_unregister_scope(fq)` → kernel's `state.handle_unregister` runs

The kernel needs to deduplicate: if `state.handle_unregister` finds that focus has already been moved (i.e., `focus_by_window[window] != fq`), it's a no-op. This already happens naturally — `handle_unregister` returns `None` when the lost FQ isn't currently focused.

### Divergence diagnostic

Dev mode: assert that the FocusChangedEvent emitted by `spatial_focus_lost` matches what `state.handle_unregister` would have computed. Log divergence.

## Tests

- Mount layer with focused scope, unmount the focused scope, assert focus moves to a sensible neighbor (matches existing handle_unregister test cases).
- Coexistence: both `spatial_focus_lost` and `spatial_unregister_scope` fire, only one transition is observed.
- Edge: focused scope's parent zone also unmounts in the same render → fallback walks up to layer.last_focused.
- Edge: layer's last_focused itself was unmounted earlier → fallback continues up to grandparent layer or NoFocus.

## Out of scope

- Removing `spatial_unregister_scope` and `state.handle_unregister` (step 11/12)

## Acceptance criteria

- When the focused scope unmounts, focus restoration follows the same rule cascade as today
- Both code paths (`focus_lost` and `handle_unregister`) coexist without producing duplicate events
- Dev divergence warnings at zero

## Files

- `kanban-app/src/commands.rs` — new `spatial_focus_lost` command
- `swissarmyhammer-focus/src/state.rs` — new `focus_lost` method
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — registry's delete handler emits the IPC; track `currentFocusFqRef` #stateless-nav