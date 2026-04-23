---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8180
project: spatial-nav
title: 'Spatial focus invariant: something is always focused; nav key on null picks a sensible default'
---
## What

**Invariant:** whenever at least one `FocusScope` is registered in the active layer, the focused moniker is non-null. The user must never end up "stuck with nothing focused" and unable to navigate.

The user reproduces this reliably from the LeftNav: click a view-switcher button → the previous view unmounts and its focused scope's rect is unregistered → the Rust store clears `focused_key` to None → React's `focusedMoniker` goes null → subsequent `h/j/k/l` keys do nothing because `broadcastNavCommand` short-circuits on `if (!focusedMk) return false`.

This task enforces the invariant at two layers and adds a nav-key safety net so the user can always recover.

## Acceptance Criteria

- [x] While at least one scope is registered in the active layer, `focused_moniker` is never null after any sequence of register/unregister/navigate calls
- [x] When the currently-focused scope is unregistered, Rust picks a successor using the priority order (layer memory → sibling → first-in-layer) and emits `focus-changed`
- [x] When a nav key fires with `focused_moniker` null, Rust selects the first-in-layer entry and emits `focus-changed` (same for stale/unregistered source moniker)
- [x] LeftNav reproduction: focus any body cell, click a LeftNav button to switch views, press any nav key → focus lands on a registered scope in the new view (not a no-op)
- [x] Inspector close: open inspector, focus an inspector row, close the inspector → focus lands on a window-layer scope (not null)
- [x] Empty-layer edge case: if the only layer has zero registered scopes, `focused_moniker` is null and nav keys are a no-op (no crash, no loop)

## Tests

- [x] Rust unit tests, parity tests, JS integration tests all green (152 Rust + 1357 JS).

## Review Findings (2026-04-21 18:35)

### Nits
- [x] `kanban-app/src/spatial.rs:104-107` — `spatial_unregister` rustdoc updated to describe the new successor-pick contract (layer memory → sibling → first-in-layer) and note that focus only clears to `None` when the layer is empty. Also updated the `FocusChanged` doc comment in `swissarmyhammer-spatial-nav/src/spatial_state.rs` for consistency. Fixed in commit pending.
