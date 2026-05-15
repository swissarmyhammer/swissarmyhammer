---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8a80
project: spatial-nav
title: Wire kernel write path for FocusScope.last_focused / FocusLayer.last_focused
---
## Reference

Surfaced during review of sub-task A (`01KQSEA6J8BCE1CAQ1S9XK7TFF`, the FocusZone collapse) on 2026-05-03. The unified-primitive collapse propagated `last_focused` from zones-only to every `FocusScope`, making the pre-existing latent bug more visible: the kernel never writes to either slot.

## What

`FocusScope.last_focused` and `FocusLayer.last_focused` are documented (now in softened form, per sub-task A's review fix) as "reserved; populated externally by the focus tracker once the wire-up exists." There is no kernel writer today — `register_scope` only *preserves* an existing value across re-registration; nothing originates one. Consequence: the `FallbackParentZoneLastFocused` and `FallbackParentLayerLastFocused` cascade arms in `state.rs::resolve_fallback` are unreachable in production. Only test fixtures that hand-populate the slot exercise them.

Implement the write hook so the documented contract becomes real:

- In `SpatialState::focus`, after the focus slot transitions to a new FQM, walk up the scope ancestor chain (via `parent_zone`) updating each ancestor's `last_focused = Some(focused_fq)`.
- Continue past the scope root into the layer chain: each ancestor `FocusLayer.last_focused = Some(focused_fq_at_that_layer_or_descendant)`.
- The walk terminates at the window root.
- Decide whether mutation lives on the registry (mutable path through `&mut SpatialRegistry`) or on a side-table held by `SpatialState` itself. The registry currently exposes scopes immutably to `state.rs`; this may require either a new `record_focus(fq)` registry mutator or relocating `last_focused` out of the immutable scope/layer structs.

## Acceptance Criteria

- [x] `SpatialState::focus` (and any other code path that mutates `focus_by_window`) updates the `last_focused` slot on every scope ancestor and every layer ancestor of the new focus.
- [x] Existing tests that hand-populate `last_focused` still pass (the kernel writes don't fight them).
- [x] New integration test in `swissarmyhammer-focus/tests/` that focuses a deeply nested scope, then unregisters it, and verifies the fallback resolves via `FallbackParentZoneLastFocused` (NOT `FallbackParentZoneNearest`) — i.e. the cascade arm becomes reachable in production.
- [x] Equivalent test for the layer arm: focus a scope inside a child layer, dismiss the layer, verify `FallbackParentLayerLastFocused`.
- [x] Tighten the docstrings on `FocusScope.last_focused` (`scope.rs`) and `FocusLayer.last_focused` (`layer.rs`) back to active voice — "populated by the kernel as focus moves" — once the writer exists. Restore the matching wording in `same_shape_layer` (`registry.rs`) too.
- [x] `cargo test -p swissarmyhammer-focus` passes; `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` clean.

## Implementation Notes

**Design choice: Option 1 — registry-side `record_focus` mutator.** Picked Option 1 over Option 2 (relocating `last_focused` to a `SpatialState` side-table) because:

1. The `last_focused` field is intrinsically per-scope/per-layer memory — moving it to a side-table would split a single concept across files and force every `FocusScope`/`FocusLayer` reader (the resolver, `register_scope`'s preservation logic, `drill_in`, `same_shape_layer`, and the test fixtures) to consult a separate map.
2. The kanban-app caller (`with_spatial`) already locks `&mut SpatialRegistry`; the immutable-registry path on `state.focus` was a self-imposed API constraint, not a structural one.
3. Switching three method signatures (`focus`, `handle_unregister`, `navigate_with`) from `&SpatialRegistry` to `&mut SpatialRegistry` was a mechanical search-and-replace across tests. The `resolve_fallback` reader remains `&SpatialRegistry`.

**Walk semantics** (`SpatialRegistry::record_focus` in `src/registry.rs`):

1. **Scope phase** — climb `parent_zone` from the focused scope. Each visited ancestor scope's `last_focused = Some(fq)`. Terminates when an ancestor's `parent_zone` is `None` (sits under layer root) or names an unregistered FQM (torn state).
2. **Layer phase** — climb the layer ancestor chain starting from the focused scope's `layer_fq` (the scope's own layer plus every ancestor reachable via `FocusLayer::parent`). Each visited layer's `last_focused = Some(fq)`. Terminates at the window root or at a missing layer reference.

The lost FQM itself is not written into its own scope's `last_focused` — that slot is reserved for descendants. A scope's own focus event reflects only on its ancestors and on its owning layer plus the layer's ancestors.

`handle_unregister` also calls `record_focus` on the fallback target so the post-unregister `last_focused` slots stay consistent with the new focus position.

`push_layer` now also preserves an existing `last_focused` across re-push (mirrors `register_scope`'s preservation logic) so StrictMode double-mounts and palette open/close cycles do not lose drill-out memory.

## Test Files

- `swissarmyhammer-focus/tests/last_focused_writer.rs` (NEW) — pins both cascade arms via the kernel writer, with closer "decoy" candidates that would win on nearest-scan. The assertions prove the resolver picked the recorded path, not the nearest.
- `swissarmyhammer-focus/tests/fallback.rs` — hand-populated `last_focused` tests now drive the resolver directly (without a prior `state.focus` call) since the writer would otherwise overwrite the fixture-populated slot. The hand-populated slot is the resolver-input under test.

## Notes

- This is independent of sub-tasks B/C/D in the spatial-nav-redesign series — those collapse the zone primitive on the React/IPC side; this is a kernel-internal fix.
- Mutability story is the design call here. Option 1: make `scopes` and `layers` `HashMap<_, RefCell<…>>` so `state.rs` can mutate them through `&SpatialRegistry`. Option 2: move `last_focused` out of the scope/layer structs entirely into a `HashMap<FullyQualifiedMoniker, FullyQualifiedMoniker>` field on `SpatialState`. Option 2 is cleaner but touches more callsites.

#spatial-nav-redesign #bug

## Review Findings (2026-05-04 08:45)

### Warnings
- [x] `swissarmyhammer-focus/src/registry.rs:1103` — `push_layer` computes `let _shape_unchanged = ... same_shape_layer(existing, &l)` but never reads the result. The leading underscore silences clippy, but the call is dead code: prior to sub-task A's collapse this gated a `scope-not-leaf` warning emission, which has since been removed. With nothing reading it, `same_shape_layer` itself is now also reachable only from this dead call site. Either delete the `_shape_unchanged` line and the now-unused `same_shape_layer` function (registry.rs:147), or, if the gate is still wanted as a future hook, keep `same_shape_layer` and add a TODO/comment explaining what it's reserved for. Today both are silently dead. The docstring on `push_layer` ("Same-shape re-registration is silent. ... The hot paths that re-push the same layer ... all flow through here repeatedly; the gate keeps them silent.") still describes the removed gate and is now misleading — drop the gate paragraph since the function unconditionally inserts.

### Nits
- [x] `swissarmyhammer-focus/src/registry.rs:1115` — the `if l.last_focused.is_none() { if let Some(existing) = ... { if existing.last_focused.is_some() { ... } } }` triple-nest mirrors `register_scope`'s preservation block but is wordier. The inner `is_some()` check is redundant — `existing.last_focused.clone()` would copy `None` harmlessly. Consider collapsing to `if l.last_focused.is_none() { if let Some(existing) = self.layers.get(&l.fq) { l.last_focused = existing.last_focused.clone(); } }` to match `register_scope`'s style. Pre-existing noise once the dead `_shape_unchanged` is removed; flag because the function is being touched here.
- [x] `swissarmyhammer-focus/src/state.rs:308` — `handle_unregister`'s docstring describes the fallback resolution and event-emission contract but does not mention the post-fallback `record_focus` call (the one that keeps `last_focused` slots in sync after the focus slot moves). The inline `// Mirror Self::focus` comment in the body covers the intent well; consider adding one sentence to the docstring (e.g. "On a successful fallback transition, this method also calls `SpatialRegistry::record_focus` on the new FQM so `last_focused` slots track the recovered focus.") so callers understand why `&mut SpatialRegistry` is required.