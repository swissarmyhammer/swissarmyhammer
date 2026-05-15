---
assignees:
- wballard
depends_on:
- 01KQW643TXM5YFKRZTNB8JPVVC
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa880
project: spatial-nav
title: 'spatial-nav redesign step 4: adapt resolve_fallback to walk snapshot when provided'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Make `resolve_fallback` (the rule cascade that picks a new focus when the current one is lost) able to run against a `NavSnapshot`. Same as step 3, but for the fallback algorithm rather than pathfinding.

## What to build

`state.rs::resolve_fallback` (and its helpers — `find_sibling_in_zone`, layer-tree walk) currently reads scope metadata from `registry.scopes` to walk parent_zone and find sibling candidates. Adapt to use the `NavScopeView` trait introduced in step 3.

For the layer-tree walk (`FallbackParentLayerLastFocused`, `FallbackParentLayerNearest`), continue reading from `registry.layers` — that part of the registry is not being removed.

For the `last_focused` reads on parent zones: in this transitional step still consult per-scope `FocusScope::last_focused`. Step 5 introduces `last_focused_by_fq` and updates the read path.

### `state.handle_unregister` adaptation

`handle_unregister` calls `resolve_fallback` and then `record_focus`. Add an `Option<&IndexedSnapshot>` parameter; when `Some`, the fallback walks the snapshot for the lost FQ's parent_zone (the lost FQ is NOT in the snapshot — already unregistered on the React side). When `None`, walks registry as today.

The snapshot path needs the lost FQ's `parent_zone` and `layer_fq` to start the walk — these are NOT in the snapshot. Solution: the IPC `spatial_focus_lost` (step 8) carries `lost_parent_zone` and `lost_layer_fq` as separate fields, OR the lost FQ remains in the snapshot just for this case (and is filtered out elsewhere). Pick one and document it.

Recommended: add two fields to the IPC for the lost case rather than overloading the snapshot. The snapshot stays "live scopes only."

## Tests

- Every existing `resolve_fallback` test gets a parallel snapshot variant.
- All five `FallbackResolution` variants exercised under both paths with identical results.
- Edge: lost FQM's `layer_fq` not in `registry.layers` → `NoFocus` both paths.
- Edge: lost FQM has no `parent_zone` (top-level scope under layer root) → walk goes straight to layer.last_focused under both paths.

## Out of scope

- `last_focused_by_fq` map (step 5)
- IPC commands (step 8)
- Removing the registry-based path (step 12)

## Acceptance criteria

- `cargo test -p swissarmyhammer-focus` green
- Both fallback paths produce matching FallbackResolution under all five variants
- `state.handle_unregister` accepts optional snapshot; without it, behavior unchanged

## Files

- `swissarmyhammer-focus/src/state.rs` — `resolve_fallback`, `handle_unregister`, fallback helpers #stateless-nav