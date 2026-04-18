---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe880
project: spatial-nav
title: 'Tauri integration test: spatial commands against real AppState'
---
## What

Every React test of the spatial system mocks `invoke` from `@tauri-apps/api/core`. Every Rust test of the spatial system calls `SpatialState` directly. **Nothing tests the wire between them.** The Tauri command handlers in `kanban-app/src/spatial.rs` — which do argument destructuring, error mapping, event emission, and `unwrap_or_default` on optional maps — are entirely untested.

### What this would catch

- Serde wire-format mismatches. React sends `navOverride: Record<string, string | null>`. Rust expects `overrides: HashMap<String, Option<String>>`. These agree *by naming convention*; a single typo on either side silently drops the field. No test catches it.
- Wrong event payload shape from Rust side.
- Missing `layer_key` on `spatial_register` — React currently skips the call when no FocusLayer, but there is no test that Rust rejects a malformed call if the skip logic ever breaks.
- `BatchEntryPayload` → `BatchEntry` mapping in `spatial_register_batch`.

### Approach

Tauri provides `tauri::test` / `MockRuntime` for unit-testing command handlers without a webview. Example pattern:

```rust
#[tokio::test]
async fn spatial_register_then_navigate_emits_event() {
    let app = tauri::test::mock_app();
    app.manage(AppState::new_for_test());
    
    let state: State<AppState> = app.state();
    let app_handle = app.handle().clone();
    
    // Subscribe to focus-changed before invoking
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    app_handle.listen("focus-changed", move |e| {
        events_clone.lock().unwrap().push(e.payload().to_string());
    });
    
    // Register two entries, focus one, navigate right
    spatial_push_layer("L1".into(), "root".into(), state.clone()).await.unwrap();
    spatial_register("k1".into(), "task:A".into(), 0.0, 0.0, 100.0, 50.0, "L1".into(), None, None, state.clone()).await.unwrap();
    spatial_register("k2".into(), "task:B".into(), 200.0, 0.0, 100.0, 50.0, "L1".into(), None, None, state.clone()).await.unwrap();
    spatial_focus("k1".into(), app_handle.clone(), state.clone()).await.unwrap();
    let next = spatial_navigate("k1".into(), "Right".into(), app_handle, state).await.unwrap();
    
    assert_eq!(next.as_deref(), Some("task:B"));
    // assert event payloads
}
```

### Subtasks

- [x] Add a test module to `kanban-app/src/spatial.rs` (or a separate `kanban-app/tests/` integration test) that uses `tauri::test::mock_app`
- [x] Cover: register → focus → navigate (happy path, emits event)
- [x] Cover: `spatial_register` with `overrides: Some(...)` that includes both a `Some(target)` redirect and a `None` block — verify navigate honors each
- [x] Cover: `spatial_register_batch` followed by `spatial_navigate` reaching a batch-registered entry
- [x] Cover: `spatial_push_layer` → register on layer 2 → `spatial_navigate` excludes layer 1 entries → `spatial_remove_layer` restores focus to layer 1's `last_focused`
- [x] Cover: `spatial_unregister` of the focused key emits `focus-changed` with `next_key: null`
- [x] Verify the event payload structure (prev_key, next_key field names) matches what the React `listen("focus-changed")` handler expects

## Acceptance Criteria

- [x] At least 6 integration tests cover the happy paths and the override path
- [x] Tests run via `cargo test -p kanban-app`
- [x] Any future change to the Rust-side wire format (field rename, type change) breaks at least one of these tests

## Implementation Notes

Rather than calling the async command functions directly (which requires `AppHandle<Wry>` and can't be satisfied by `mock_app()` → `AppHandle<MockRuntime>`), the tests drive the full IPC path using `tauri::test::get_ipc_response`. This exercises the same argument deserialization, type conversion, and event emission that a real React invoke goes through.

Two changes outside the test module were required:

1. **`kanban-app/src/spatial.rs`**: The command handlers that take `AppHandle` are now generic over `R: Runtime`. This follows the convention used by Tauri's first-party plugins (e.g. `tauri-plugin-dialog`) and is necessary so the same handlers can be registered on both a `Wry` app (production) and a `MockRuntime` app (tests). Handlers that only take `State` were not touched.
2. **`kanban-app/Cargo.toml`**: Added a `[dev-dependencies] tauri = { ... features = [..., "test"] }` override so `tauri::test::mock_app`, `mock_builder`, `MockRuntime`, and `get_ipc_response` are available in test builds only. Release builds never see the `test` feature.

Seven tests are in the new `tauri_integration_tests` module at the bottom of `kanban-app/src/spatial.rs`:

- `register_focus_navigate_emits_events_and_returns_moniker` — happy path
- `navigate_honors_override_redirect_and_block` — override map round-trip
- `register_batch_then_navigate_reaches_batch_entry` — batch entry mapping
- `push_layer_isolates_navigation_and_remove_restores_focus` — layer stack isolation + restore
- `unregister_focused_key_emits_focus_changed_with_null_next_key` — unregister emission
- `clear_focus_emits_event_with_prev_key_and_null_next` — extra coverage on clear_focus
- `focus_changed_payload_uses_snake_case_field_names` — wire-format lock-in

## Review Findings (2026-04-18 15:05)

### Nits
- [x] `kanban-app/Cargo.toml:64` — The dev-dependency comment points readers to `tests/spatial_tauri_integration.rs`, but the tests actually live in `kanban-app/src/spatial.rs` in the `tauri_integration_tests` module. Update the path in the comment so future readers don't hunt for a file that doesn't exist.