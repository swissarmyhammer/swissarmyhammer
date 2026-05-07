---
assignees:
- wballard
depends_on:
- 01KQW62XNHC1YP8ZKJGGFP0JZW
- 01KQW675YPMAW9AGV4P80Y64V3
- 01KQW6880CFYR0A04RKBSJ79Q1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffac80
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

## Review Findings (2026-05-06 16:47)

### Warnings

- [x] `kanban-app/ui/src/lib/spatial-focus-context.tsx:482-494` — Production unmount silently skips the IPC. `LayerScopeRegistry.delete(fq)` is called from a `useEffect` cleanup in `focus-scope.tsx:439-445`. By the time that cleanup runs, React has already invoked `setRef(null)` (from `focus-scope.tsx:330-341`) during the commit phase, so `entry.ref.current` is `null` and `node?.getBoundingClientRect()` returns `undefined`. The handler then hits `if (domRect === undefined) return;` and never reaches `invoke("spatial_focus_lost", ...)`. The acceptance criterion "When the focused scope unmounts, focus restoration follows the same rule cascade as today" is met *only* by the surviving `spatial_unregister_scope` path; the new IPC is dead code in real unmount flows. The existing test in `spatial-focus-lost.test.tsx` does not exercise this — it calls `registry.delete(fq)` directly while `entry.ref.current` still points at a live (detached) `document.createElement("div")` whose `getBoundingClientRect()` returns `{0,0,0,0}` (non-undefined). Suggested fix: capture and cache the rect on the entry during registration / on `setRef` transitions, then read it from the cached value at delete time. Alternative: subscribe to `delete` from a `useLayoutEffect` cleanup so it runs before React clears the ref. Either way, add a regression test that drives an actual `unmount()` from `@testing-library/react` and asserts the IPC fires with a non-zero rect.

### Nits

- [x] `kanban-app/ui/src/lib/layer-scope-registry.test.tsx:163-209` — The new `onDeleted` tests cover registration / fire-on-delete, silent-on-unknown, and unsubscribe, but not "listener exception isolation". The implementation in `layer-scope-registry-context.tsx:170-176` explicitly catches and logs listener exceptions; that contract is currently unpinned. Add a test that registers a listener that throws, deletes a registered FQ, and asserts (a) the second listener still ran, and (b) `console.error` was called with the `[LayerScopeRegistry] deleted listener threw` prefix.

- [x] `kanban-app/src/commands.rs:69-79` — `check_focus_lost_divergence`'s doc comment ("Bedding-in instrumentation matching [...]: the snapshot path is the new authoritative path for unmount-driven focus loss, but until every layer's snapshot is proven to track the registry the registry-path resolution is computed alongside so a regression is observable in dev logs.") is migration-phase narrative — it describes the bedding-in moment, not a stable contract. When the registry path is removed in step 11/12 this comment will rot. The peer functions `check_navigate_divergence` / `check_focus_divergence` may carry the same shape; flag for consistency with the repo's doc-comment rules. Suggested rewrite: one sentence stating "Compares the registry-path and snapshot-path fallback resolutions and warns on divergence; debug-only, observation-only."

- [x] `kanban-app/ui/src/lib/layer-scope-registry-context.tsx:112-119` — `ScopeDeletedListener`'s doc comment ends with "...the property the focused-scope-unmount IPC relies on", which names a specific caller. Per the project's doc-comment rules ("Don't reference [...] specific callers"), the post-delete invariant is the contract; the consuming code's reliance is incidental. Suggested rewrite: drop the trailing clause, leaving "Fires AFTER the entry leaves the underlying `Map`, so a snapshot built inside the callback correctly excludes the lost FQM."

- [x] `swissarmyhammer-focus/src/state.rs:32-80` — `focus_lost` re-implements the focus-commit dance (insert into `focus_by_window`, call `record_focus`, build `FocusChangedEvent`) instead of routing through `focus_with_snapshot` as the parent card's "Commits via `state.focus_with_snapshot`" sub-bullet specified. Functionally equivalent here because the `FallbackResolution` already supplies `next_segment`, but the duplication will silently drift when one path gets a contract change the other doesn't. Suggested fix: factor the fallback-commit into a `focus_with_snapshot_for_next`-style helper that both `handle_unregister` and `focus_lost` call, or accept the duplication and add a comment that the two `match` arms must stay in lockstep.

### Resolution notes (2026-05-06)

- **Warning fix:** Cached the bounding rect on `ScopeEntry` as a mutable `lastKnownRect: Rect | null`. The cache is seeded on layer-registry `add`, then refreshed at every rect-sample site (`<FocusScope>`'s mount-time `getBoundingClientRect()`, the ResizeObserver fire path in `<FocusScope>`, and the rAF-throttled scroll path in `useTrackRectOnAncestorScroll`). The deletion listener now reads `entry.lastKnownRect` rather than calling `getBoundingClientRect()` on a ref that React has already nullified, and skips the IPC only when no rect was ever sampled. New `LayerScopeRegistry.updateRect` method exposes the cache write to callers.
- **Real-unmount regression test:** `kanban-app/ui/src/lib/spatial-focus-lost.test.tsx` now contains a `spatial_focus_lost real unmount lifecycle` block that mounts a real `<FocusScope>` inside a `<FocusLayer>`, sets focus on it, then re-renders without the scope (the production scenario — parent layer stays alive). The test asserts the IPC fires with the cached non-zero rect. A whole-tree `unmount()` is *not* the right shape: when the layer unmounts in the same commit, its `registerLayerRegistry` cleanup unsubscribes the deletion listener before the child scope's cleanup runs `delete()`, so the listener correctly does not fire — production unmounts almost always keep the enclosing layer alive (a column shrinking, an inspector row vanishing) and the regression test pins that case.
- **Listener exception isolation test:** Added in `layer-scope-registry.test.tsx`; registers a throwing listener and a surviving listener, deletes a registered FQ, and asserts the surviving listener still ran plus `console.error` was called with the documented `[LayerScopeRegistry] deleted listener threw` prefix.
- **Doc-comment rewrites:** `check_navigate_divergence`, `check_focus_divergence`, and `check_focus_lost_divergence` now carry one-sentence summaries describing the comparison and the debug-only / observation-only nature, with the migration-phase narrative removed. `ScopeDeletedListener`'s trailing "the property the focused-scope-unmount IPC relies on" clause is dropped.
- **Shared fallback-commit helper:** Introduced `SpatialState::commit_fallback_resolution` in `swissarmyhammer-focus/src/state.rs`. Both `handle_unregister` and `focus_lost` now route through it; the only difference between the two callers is whether `record_focus` reads from a snapshot view or the registry. The match arms can no longer drift independently when contracts change.
