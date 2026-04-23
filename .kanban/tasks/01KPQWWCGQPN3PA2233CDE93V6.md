---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff880
project: spatial-nav
title: 'FocusLayer: auto-focus first (upper-left) entry on layer push'
---
## What

When a `FocusLayer` mounts (inspector, future modals, or even the root window layer), nothing inside it is focused by default. `spatial_push_layer` updates the layer stack but leaves `focused_key` pointing at whatever was focused in the outer layer — which `spatial_navigate` then filters out because it doesn't belong to the new active layer. User effect: inspector opens, user presses `j`, focus doesn't move (or falls back to `First` via the missing-source-key path which happens to do the right thing by accident).

Make layer push auto-focus the first (upper-left, via the existing `First` direction selector) registered entry in the new layer. This makes layer semantics symmetric with `remove_layer`, which already restores the outgoing layer's `last_focused`.

Today's per-view workarounds (`useGridInitialFocus` in `grid-view.tsx:111`, `initialFocusDone` + `setFocus` in `board-view.tsx:782`) become redundant for the window layer — they can be deleted in a follow-up once this lands. Inspector currently has no initial-focus code at all; this task adds it implicitly.

### Architecture

**Rust (`swissarmyhammer-spatial-nav/src/spatial_state.rs`)**: new `SpatialState::focus_first_in_layer(&self, layer_key: &str) -> Option<FocusChanged>`. Finds the `First` entry (top-leftmost by y then x) whose `layer_key` matches, sets `focused_key` to its key, saves prior focus via existing `save_focus_memory` path, returns the event. No-op if the layer has zero entries or if `focused_key` is already in that layer (don't override manual focus).

**Tauri command (`kanban-app/src/spatial.rs`)**: new `spatial_focus_first_in_layer(layer_key: String, window: WebviewWindow<R>, state: State<AppState>)`. Looks up the window's `SpatialState`, calls the method above, emits `focus-changed` via `window.emit_to(window.label(), ...)` if an event came back. Register in `main.rs` invoke handler.

**JS shim (`kanban-app/ui/src/test/spatial-shim.ts`)**: matching `focusFirstInLayer(layerKey: string)` that mirrors Rust. `setup-spatial-shim.ts` dispatches `spatial_focus_first_in_layer` to it.

**React (`kanban-app/ui/src/components/focus-layer.tsx`)**: `useLayerRegistration` invokes `spatial_focus_first_in_layer` on a `requestAnimationFrame` after `spatial_push_layer` — the RAF lets descendant `FocusScope` effects (which run bottom-up before the parent `FocusLayer` effect) register their rects before the First selector runs. Cancel on cleanup.

**Parity fixture**: new case in `kanban-app/ui/src/test/spatial-parity-cases.json` — push layer L1 empty (expect no event), register two entries (0,0) and (200,0), `focus_first_in_layer L1` (expect event prev=null next=first), focus the second entry, `focus_first_in_layer L1` again (expect no event — already focused in the layer). Both the JS shim and Rust `tests/parity.rs` exercise it.

### Scope decision: NOT touching window-layer auto-focus

This task ONLY adds the mechanism and wires it into FocusLayer. The per-view workarounds in board-view and grid-view stay in place for now (removing them is a separate cleanup task). Inspector gains initial focus as a natural consequence of the FocusLayer change. This keeps the blast radius small and avoids co-mingling cleanup with a behavior addition.

### Edge cases

- **Layer push before any entry registers**: RAF-deferred invoke will find an empty layer and no-op. Next registration does not retroactively focus — accepted trade-off. If this matters for a future view with async data, the view component can call `spatial_focus_first_in_layer` explicitly after data loads.
- **User clicks before RAF fires**: user's click calls `setFocus(...)` → `spatial_focus(key)` → `focused_key` updated. By the time our deferred `focus_first_in_layer` runs, `focused_key` is already in the active layer, and the early-exit no-op kicks in.
- **Layer push on an already-focused layer (remount)**: `focus_first_in_layer` short-circuits if `focused_key` is already in that layer.

## Acceptance Criteria

- [x] New method `SpatialState::focus_first_in_layer(&str) -> Option<FocusChanged>` with doc comment
- [x] New Tauri command `spatial_focus_first_in_layer(layer_key)` registered in `main.rs` invoke handler
- [x] JS shim has matching `focusFirstInLayer` exercised by `setup-spatial-shim.ts`
- [x] `FocusLayer` invokes the new command via RAF after push; cleanup cancels the RAF
- [x] Opening the inspector in the running app focuses the first field (manual smoke) — covered by new vitest-browser regression test
- [x] No regression: `focus-scope.test.tsx`, `spatial-nav-canonical.test.tsx`, `spatial-nav-{board,grid,inspector,leftnav,perspective}.test.tsx` all green
- [x] Parity case added; both `spatial-shim-parity.test.ts` (JS) and `swissarmyhammer-spatial-nav/tests/parity.rs` (Rust) pass

## Tests

### Rust unit tests (`swissarmyhammer-spatial-nav/src/spatial_state.rs`)

- [x] `focus_first_in_layer_returns_first_entry_by_y_then_x` — three entries in one layer at (100,50), (0,0), (50,0). Call `focus_first_in_layer`. Asserts focused_key matches the (0,0) entry.
- [x] `focus_first_in_layer_noop_when_already_focused_in_layer` — register two entries, focus second, call method, returns None and focused_key unchanged.
- [x] `focus_first_in_layer_skips_entries_in_other_layers` — layer A has one entry, layer B has two; calling `focus_first_in_layer("A")` returns A's entry, not B's.
- [x] `focus_first_in_layer_empty_layer_returns_none` — push layer, no register calls, call method. Returns None, focused_key unchanged.
- [x] `focus_first_in_layer_saves_prior_focus_memory` — register in layer A, focus it, push layer B with entries, `focus_first_in_layer("B")`. Layer A's `last_focused` equals the prior key.

### Tauri integration test (`kanban-app/src/spatial.rs` under `tauri_integration_tests`)

- [x] `spatial_focus_first_in_layer_emits_focus_changed_scoped_to_window` — build mock app with window "A", push layer, register two entries, invoke command, assert focus-changed was emitted only to A with the expected next_key.

### Parity test

- [x] New case in `spatial-parity-cases.json` per the Scope section above; runs in both `spatial-shim-parity.test.ts` and `swissarmyhammer-spatial-nav/tests/parity.rs`.

### Vitest-browser test (`kanban-app/ui/src/test/spatial-nav-inspector.test.tsx`)

- [x] New `it("opening the inspector auto-focuses the first field", ...)` — use the existing inspector fixture; mount with inspector closed, trigger inspector open, assert without any keystroke that `data-focused="true"` is on the first field's FocusScope element. This is the user-visible regression guard.

### Test commands

- [x] `cargo test -p swissarmyhammer-spatial-nav` — 5 new unit tests green
- [x] `cargo test -p kanban-app tauri_integration_tests` — 1 new integration test green
- [x] `cd kanban-app/ui && npm test` — all test files green including the new inspector assertion

## Workflow

- Use `/tdd` — write failing tests first for each tier (Rust unit, Tauri integration, parity, vitest-browser), then implement the method, command, shim, and FocusLayer wiring until each test goes green.
- Land tests and implementation together — do not ship the Rust method without the shim equivalent, or vice versa (drift breaks every consumer of the harness).