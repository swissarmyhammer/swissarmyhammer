---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff180
project: spatial-nav
title: 'Multi-window: per-window SpatialState so nav doesn''t leak between windows'
---
## What

With two (or more) kanban windows open simultaneously, pressing `h/j/k/l` in window A causes focus to jump to something in window B. Confirmed by direct manual observation.

### Root cause

The spatial registry is a process-global — `AppState.spatial_state: SpatialState` is a single instance, shared by Tauri across every window it manages. Consequently:

- **Registration**: every FocusScope from every window calls `spatial_register` into the same `HashMap<key, SpatialEntry>`. Window A's cell rects and window B's cell rects are mixed into one pool.
- **Layer stack**: the app-level `FocusLayer name="window"` mounts in every window with a new ULID, pushing both into the SAME `LayerStack`. `active_layer()` returns whichever was pushed last — cross-window order of operations decides whose focus "wins".
- **Navigation**: `spatial_navigate(key, dir)` filters by the active layer but the active layer may belong to a window different from the one the user typed in. The beam test then picks candidates in that other window.
- **Events**: `app.emit("focus-changed", …)` is an app-wide broadcast. Every window's `listen("focus-changed")` handler fires — so window A's focus changes repaint claim callbacks in window B too.
- **Cross-window coupling on `setFocus`**: React's `useFocusSetter` invokes `spatial_focus` on a key it pulled out of `monikerToKeysRef`, which contains entries from any window that registered that moniker. The pick is arbitrary.

The `<FocusLayer name="window">` wrap was meant to isolate per-window nav, but naming a layer "window" doesn't actually scope its identity by window — it's just another key in one shared stack.

### Scope of impact

Every in-flight spatial-nav task assumes single-window semantics and must be revisited:

- `01KPNWF3WM` — Revert in-session edits — **unaffected** (it's about the frontend code state, not the registry model).
- `01KPNWFNW7` — Forgiving serde — **unaffected** (arg shape, not routing).
- `01KPNWGFTF` — E2E harness — **needs multi-window E2E**: add a scenario with two windows open and assert that `j` in window A does not move focus in window B.
- `01KPNWH82X` / `01KPNWNEN1` / `01KPNWPEMK` / `01KPNWP1KA` — any nav E2E must also run the two-windows variant or explicitly note single-window-only scope.
- `01KPNWHP4Z` / `01KPNWHZJS` / `01KPNWPX9N` / `01KPNWQ844` — reachability to LeftNav and perspective bar must be scoped to the active window; if window B's LeftNav is beam-test-closer-in-pixels than window A's, we'd jump there without this fix.
- `01KPNWJKPH` — "jumps to first and sticks" — likely interacts with multi-window: if the "first" entry picked by the Rust fallback happens to be in a different window, it looks like teleportation.
- `01KPGXAWD5` — `__spatial_dump` cfg gates — gate itself fine, but the dump output needs per-window slicing for tests.

### Target architecture

Option A (recommended): **Per-window `SpatialState` instance.** AppState becomes `HashMap<WindowLabel, Arc<SpatialState>>` (or `DashMap` if concurrent access matters). Tauri commands take a `Window<R>` parameter and look up the state for that specific window. `focus-changed` is emitted via `window.emit(...)` rather than `app.emit(...)` so only the originating webview hears it.

Option B: **Window-aware layer keys.** Keep one `SpatialState` but include the window label in every registered entry (new `window_label: String` field on `SpatialEntry`). Filter candidates to the window matching the source key. More surgery inside `spatial_state.rs` but no structural change in `AppState`.

Option A is the cleaner story — fewer places need window-awareness; A/B/C testing simpler. Going with A unless Tauri's per-window state ergonomics get ugly.

### TDD — failing tests

At the Rust level (`swissarmyhammer-spatial-nav` unit tests can't help — they don't know about windows). These belong in `kanban-app/src/spatial.rs`'s `tauri_integration_tests`:

```rust
#[tokio::test]
async fn two_windows_register_independently() {
    let app = mock_app_with_two_windows("A", "B");
    // In window A, push a layer + register an entry at (0, 0).
    invoke_in_window(&app, "A", "spatial_push_layer", ...).await;
    invoke_in_window(&app, "A", "spatial_register", &json!({
        "key": "a1", "moniker": "task:a", "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0,
        "layerKey": "LA", "parentScope": null, "overrides": null,
    })).await;
    // In window B, register an entry at (0, 100).
    invoke_in_window(&app, "B", "spatial_push_layer", ...).await;
    invoke_in_window(&app, "B", "spatial_register", &json!({
        "key": "b1", "moniker": "task:b", "x": 0.0, "y": 100.0, "w": 10.0, "h": 10.0,
        "layerKey": "LB", "parentScope": null, "overrides": null,
    })).await;

    // __spatial_dump in window A sees only a1 — not b1.
    let dump_a = invoke_in_window(&app, "A", "__spatial_dump", &json!({})).await.unwrap();
    assert_eq!(dump_a["entry_count"], 1);
    assert_eq!(dump_a["layer_stack"][0]["entry_count_in_layer"], 1);

    // Same for window B — only b1.
    let dump_b = invoke_in_window(&app, "B", "__spatial_dump", &json!({})).await.unwrap();
    assert_eq!(dump_b["entry_count"], 1);
}

#[tokio::test]
async fn navigate_in_one_window_does_not_emit_events_to_the_other() {
    // Set up two windows, register in both, focus in window A.
    // Subscribe to focus-changed on BOTH windows.
    // Invoke spatial_navigate in window A.
    // Assert window A received a focus-changed event AND window B did not.
}

#[tokio::test]
async fn spatial_navigate_from_window_a_cannot_return_window_b_candidates() {
    // Window A has an entry at (0,0) and one at (200,0). Window B has one at (0, 50).
    // Focus on (0,0) in window A. spatial_navigate Down.
    // Assert returned moniker is from window A (either (200,0) clamp or no move), not window B's (0,50).
}
```

All three must fail against HEAD.

### Subtasks

- [x] Add `mock_app_with_two_windows` helper in `kanban-app/src/test_support.rs` (or equivalent) for Tauri integration tests — added inside `tauri_integration_tests` module as `build_test_app_with_windows(&[...])` plus `invoke_in_window`
- [x] Write the three failing tests — `two_windows_register_independently`, `navigate_in_one_window_does_not_emit_events_to_the_other`, `spatial_navigate_from_window_a_cannot_return_window_b_candidates`
- [x] Refactor `AppState` to own per-window `SpatialState` (Option A) — lookup by window label in each command handler — `spatial_states: RwLock<HashMap<String, Arc<SpatialState>>>` + `spatial_state_for(label)` helper
- [x] Route `focus-changed` emissions through `window.emit` rather than `app.emit` — using `window.emit_to(window.label(), …)` which is the Tauri-idiomatic way to scope (plain `window.emit` is identical to `app.emit` in Tauri v2)
- [x] Update `__spatial_dump` to return per-window data — takes `WebviewWindow<R>`, dumps the state for that window only
- [x] Re-run every existing spatial nav test; all should still pass — all 88 kanban-app unit tests pass including 14 existing spatial tests + 3 new multi-window tests
- [ ] Manual verification in the app with two windows — deferred: see follow-up 01KPP5S6T64SFYWRCHTW1DNNM1 for the required frontend change (entity-focus-context.tsx must switch from app-wide `listen()` to `getCurrentWebviewWindow().listen()` for UI-level isolation)

### Acceptance

- [x] All three new failing tests pass
- [x] Existing single-window tests green
- [ ] Manual: two windows open, h/j/k/l in window A never moves focus in window B — requires follow-up 01KPP5S6T64SFYWRCHTW1DNNM1
- [ ] Manual: closing one window doesn't leak its entries into the other's registry — Rust side verified by `remove_spatial_state` in `on_window_destroyed`; manual UI check still pending
- [x] `__spatial_dump` shows per-window counts — verified by `two_windows_register_independently`

### Follow-up

After this lands, every in-flight spatial-nav task needs a multi-window E2E variant or an explicit "single-window scope" note. I'll annotate each as this task progresses.

Also: 01KPP5S6T64SFYWRCHTW1DNNM1 (frontend follow-up) tracks updating `entity-focus-context.tsx` to use `getCurrentWebviewWindow().listen()` — without it, the Rust side is correctly scoped but UI listeners registered with default `target: Any` still fire in every window.