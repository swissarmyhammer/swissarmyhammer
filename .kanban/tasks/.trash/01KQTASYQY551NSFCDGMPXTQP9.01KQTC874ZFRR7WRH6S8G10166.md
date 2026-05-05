---
assignees:
- wballard
position_column: todo
position_ordinal: d080
title: 'spatial-nav: cascade-drop descendants on unregister_scope / remove_layer'
---
## Problem

The spatial-nav kernel accumulates stale entries because it does not cascade-drop descendants when a parent unregisters. Symptom: `check_overlap_warning` logs `two entries share (x, y)` for two different tasks' project chips at the same coordinate — one is the live chip, the other is a kernel ghost from a prior render.

Example log:

```
op="update_rect"
new_fq=/window/.../task:01KQSE8.../field:....project/project:spatial-nav
overlap_fq=/window/.../task:01KQSF0.../field:....project/project:spatial-nav
x=121 y=674
```

Two tasks at different positions in the same column physically cannot share a y-coordinate, so one entry is stale.

## Root cause

The kernel trusts the React side to unmount every descendant before the parent and to deliver every unregister IPC. Both assumptions are wrong in failure cases.

- `SpatialRegistry::unregister_scope` (`swissarmyhammer-focus/src/registry.rs:518-528`) removes a single FQ. No descendant walk.
- `SpatialRegistry::remove_layer` (`swissarmyhammer-focus/src/registry.rs:1116-1121`) removes a single layer. Doc-comment explicitly states: *"Does not cascade to scopes that name this layer in their `layer_fq` — the React side unmounts those scopes first via `spatial_unregister_scope`."*
- React-side cleanups (`focus-scope.tsx:389-394`, `focus-layer.tsx:209-213`) call `.catch(console.error)` — IPC failures are silent.

If any descendant's unregister IPC is missed (error boundary tearing the subtree, async race, dispatch drop), that scope orphans permanently. Every subsequent `update_rect` against neighbors triggers `check_overlap_warning` against the ghost.

## Fix

Make the kernel authoritative for subtree cleanup.

1. **`unregister_scope(fq)`**: after removing `fq` from `self.scopes`, walk `self.scopes` and drop every scope whose `parent_zone == fq` (transitively — BFS/DFS, not just direct children). Also clear their `overlap_warn_partner` slots and `validated_layers` entry for the affected layer.
2. **`remove_layer(fq)`**: drop every scope whose `layer_fq == fq`. Update the doc-comment — kernel is now authoritative; React-side parity is no longer assumed.
3. Add a `debug_assert!` that, on `remove_layer` of a window-root layer, `self.scopes` contains no scope with that `layer_fq` afterward — future leaks fail loudly in tests.
4. Optional but recommended: add a `tracing::debug!` count on each cascade ("cascade dropped N descendants under fq=...") so production logs reveal the real leak rate.

Both walks are O(scopes), scopes are visible-UI-bounded, runs only on unmount — cheap.

## Tests

- New unit test: register parent zone + leaf scope; call `unregister_scope(parent)`; assert leaf is also gone.
- New unit test: register layer + scope under it; call `remove_layer`; assert scope is gone.
- New unit test: register parent + child + grandchild; unregister parent; assert all three are gone (transitive).
- Add an `overlap_tracing` integration check that mounting/unmounting a kanban-card with chips a few thousand times leaves `scopes` empty modulo the persistent layer/zone roots.

## Out of scope

- React-side instrumentation for register/unregister parity — separate task if needed.
- Generation/epoch GC sweep — cascade fix should remove the need; revisit only if leaks reappear.

## Files

- `swissarmyhammer-focus/src/registry.rs` — `unregister_scope` (~line 518), `remove_layer` (~line 1116)
- `swissarmyhammer-focus/tests/focus_registry.rs` — new tests
- `swissarmyhammer-focus/tests/overlap_tracing.rs` — leak-resistance integration test

## Reference

- React cleanup paths (verified present): `kanban-app/ui/src/components/focus-scope.tsx:389-394`, `kanban-app/ui/src/components/focus-layer.tsx:209-213`
- Original symptom: `kanban-app[49073]` log on 2026-05-04 with `op="update_rect"` overlap warning between two task project chips.