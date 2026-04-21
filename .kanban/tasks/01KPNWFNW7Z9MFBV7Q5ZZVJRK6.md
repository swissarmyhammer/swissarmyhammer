---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffee80
project: spatial-nav
title: Rust spatial_register command must accept both snake_case and camelCase arg names (forgiving serde)
---
## What

This session burned an hour discovering that JS-side `invoke("spatial_register", { layer_key: ... })` silently fails because Tauri v2 defaults to camelCase arg names. The existing Rust integration tests use `"layerKey"` and pass; frontend code used `"layer_key"` and silently dropped every registration. The `.catch(() => {})` swallowed the error.

The user's explicit direction: **"we need nice forgiving serde aliases and desers"**. Tauri commands should accept either naming convention so a future developer who types the Rust-natural `layer_key` doesn't silently break focus.

### Scope (which commands)

All spatial commands in `kanban-app/src/spatial.rs` that have multi-word args:

- `spatial_register` — `layer_key`, `parent_scope`
- `spatial_register_batch` — the `BatchEntryPayload` struct has `layer_key`, `parent_scope`

Other spatial commands have single-word args (`key`, `name`, `direction`) and are unaffected.

### TDD — failing tests first

Add to `tauri_integration_tests` in `kanban-app/src/spatial.rs`:

```rust
#[tokio::test]
async fn spatial_register_accepts_snake_case_arg_names() {
    // Fails today: Tauri rejects the snake_case arg and the command errors.
    let app = /* mock_app */;
    let payload = json!({
        "key": "k1",
        "moniker": "task:a",
        "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0,
        "layer_key": "L1",   // ← snake_case
        "parent_scope": null,
        "overrides": null,
    });
    let result = invoke_command(&app, "spatial_register", payload).await;
    assert!(result.is_ok(), "snake_case should be accepted: {:?}", result);
}

#[tokio::test]
async fn spatial_register_accepts_camel_case_arg_names() {
    // Passes today — keeps passing.
    let app = /* mock_app */;
    let payload = json!({
        "key": "k1",
        "moniker": "task:a",
        "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0,
        "layerKey": "L1",    // ← camelCase
        "parentScope": null,
        "overrides": null,
    });
    assert!(invoke_command(&app, "spatial_register", payload).await.is_ok());
}
```

And matching pairs for `spatial_register_batch`'s `BatchEntryPayload` fields.

### Approach (after tests fail)

Two clean options; pick whichever tests prove is correct:

**A. `#[tauri::command(rename_all = "snake_case")]`** — makes Tauri EXPECT snake_case on the wire. Breaks existing camelCase callers including the current tests. Not forgiving.

**B. Wrap args in a dedicated `Deserialize` struct with serde rename aliases.** Example:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpatialRegisterArgs {
    key: String,
    moniker: String,
    x: f64, y: f64, w: f64, h: f64,
    #[serde(alias = "layer_key")]
    layer_key: String,
    #[serde(alias = "parent_scope", default)]
    parent_scope: Option<String>,
    #[serde(default)]
    overrides: Option<HashMap<String, Option<String>>>,
}

#[tauri::command]
pub async fn spatial_register(
    args: SpatialRegisterArgs,
    state: State<'_, AppState>,
) -> Result<(), String> { ... }
```

Option B is the "forgiving" approach the user asked for. Apply the same pattern to `BatchEntryPayload` (the struct already exists; add `#[serde(rename_all = "camelCase")]` with aliases).

### Acceptance

- [ ] Both test pairs pass (snake_case AND camelCase accepted) for `spatial_register` and `spatial_register_batch` entries
- [ ] Existing tests unchanged; CI green
- [ ] Frontend `focus-scope.tsx` invoke unchanged (it can use either naming now) — leaving the current camelCase form is fine