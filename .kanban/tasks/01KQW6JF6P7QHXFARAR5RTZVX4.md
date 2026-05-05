---
assignees:
- wballard
depends_on:
- 01KQW6H3397154YJWDPD6TDYZ3
position_column: todo
position_ordinal: db80
project: spatial-nav
title: 'spatial-nav redesign step 12: cutover (3/4) — shrink SpatialRegistry, delete scopes map and per-scope last_focused'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**. Third of four cutover steps.

## Goal

Delete the kernel-side scope registry and everything that depended on it. After this step, `SpatialRegistry` only holds layers, last_focused_by_fq, and focus_by_window. The `Option<&NavSnapshot>` parameter on pathfinding/fallback/record_focus becomes a required `&NavSnapshot`.

## What to delete

### Fields and methods on SpatialRegistry

`swissarmyhammer-focus/src/registry.rs`:

- Delete field: `scopes: HashMap<FullyQualifiedMoniker, FocusScope>`
- Delete struct: `FocusScope` (the kernel-side per-scope record — not to be confused with the React `<FocusScope>` component, which keeps its name)
- Delete methods: `register_scope`, `unregister_scope`, `update_rect`, `find_by_fq`, `check_overlap_warning`
- Delete fields: `validated_layers`, `overlap_warn_partner`
- The `last_focused: Option<FullyQualifiedMoniker>` field that was on the per-scope record dies with the struct

`last_focused_by_fq` (added in step 5) becomes the sole storage. Remove the dual-write code from `record_focus`; only the map is written. Remove the per-scope-fallback read code from `resolve_fallback`; only the map is read.

### State methods

`swissarmyhammer-focus/src/state.rs`:

- Delete `state.handle_unregister` (replaced by `focus_lost` in step 8)
- Delete `state.resolve_fallback` registry-path branch
- Make `Option<&NavSnapshot>` parameters required `&NavSnapshot` everywhere (`focus`, `navigate`, `record_focus`, `resolve_fallback`)

### Pathfinding

`swissarmyhammer-focus/src/navigate.rs`:

- Delete the `NavScopeView` impl for `&SpatialRegistry` (registry no longer has a scopes map to iterate)
- Keep the trait + the snapshot impl
- Or simpler: remove the trait entirely now that there's only one impl, inline `IndexedSnapshot` access into pathfinding

Recommended: drop the trait. Replace with direct `&IndexedSnapshot` arguments. Cleanest result.

## What survives

- `layers: HashMap<FQM, FocusLayer>`
- `last_focused_by_fq: HashMap<FQM, FQM>`  ← was added in step 5, now sole truth
- `focus_by_window: HashMap<WindowLabel, FQM>` (lives on `SpatialState`)
- Pathfinding (`geometric_pick`, `BeamNavStrategy`) — takes `&IndexedSnapshot`
- `resolve_fallback` — takes `&IndexedSnapshot`
- `record_focus` — takes `&IndexedSnapshot`
- `state.focus`, `state.navigate`, `state.focus_lost`, `state.clear_focus`
- Layer ops: `push_layer`, `pop_layer`, `remove_layer`

## Tests

- Every kernel test that previously built scopes via `registry.register_scope(...)` is rewritten to build a `NavSnapshot` directly and pass it to the kernel call. (Many tests will need touch-up; this is expected.)
- The full `cargo test -p swissarmyhammer-focus` suite passes against the slimmed kernel.
- e2e tests on the React side stay green — they were already exercising the snapshot path.

## Out of scope

- Moving overlap warning to JS (step 13)

## Acceptance criteria

- `SpatialRegistry` is significantly smaller; no `scopes` field, no per-scope `last_focused`
- All snapshot parameters that were `Option<&NavSnapshot>` are now required `&NavSnapshot`
- `cargo test -p swissarmyhammer-focus` green
- `pnpm -C kanban-app/ui test` green

## Files

- `swissarmyhammer-focus/src/registry.rs` — major shrink
- `swissarmyhammer-focus/src/state.rs` — delete handle_unregister, tighten signatures
- `swissarmyhammer-focus/src/navigate.rs` — drop trait, take `&IndexedSnapshot`
- `swissarmyhammer-focus/tests/*` — rewrite scope-building helpers to use snapshots #01KQTC1VNQM9KC90S65P7QX9N1