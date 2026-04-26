---
assignees:
- claude-code
depends_on:
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
- 01KNQXXF5W7G4JP73C6ZCMKYKX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffac80
project: spatial-nav
title: 'Tauri spatial-nav adapter foundation: wire SpatialState/SpatialRegistry into AppState, add core commands, register handlers'
---
## What

Establish the **Tauri adapter foundation** for spatial navigation. The headless `swissarmyhammer-focus` crate (kernel work — `SpatialState`, `SpatialRegistry`, `BeamNavStrategy`, `FallbackResolution`, `RegisterEntry`, drill-in/drill-out) is complete and exhaustively tested at the Rust level. But there is **no Tauri command surface** wiring the kernel into the kanban-app yet — `kanban-app/src/state.rs` has no `SpatialState`/`SpatialRegistry` field; `kanban-app/src/commands.rs` has no `spatial_*` command; `kanban-app/src/main.rs` has no spatial entries in `invoke_handler!`.

The React side (`kanban-app/ui/src/lib/spatial-focus-context.tsx`) already invokes `spatial_focus`, `spatial_register_focusable`, `spatial_register_zone`, `spatial_unregister_scope`, `spatial_update_rect`, `spatial_navigate`, `spatial_push_layer`, `spatial_pop_layer`, `spatial_drill_in`, `spatial_drill_out` — all of these will fail at runtime today because the commands don't exist. Several downstream cards depend on this surface: dynamic-lifecycle (`01KNS0B3HY...`), drill-in/drill-out (`01KPZS4RG0...`), inspector layer (`01KNQXYC4R...`), board/column/card/grid zones, dialogs, navOverride cleanup. None of them can complete their UI/IPC work without this foundation.

This card establishes the foundation:

1. **AppState wiring** — `kanban-app/src/state.rs` gains `spatial_state: Arc<Mutex<SpatialState>>` and `spatial_registry: Arc<Mutex<SpatialRegistry>>` (both inside the existing `AppState` struct; matching the per-window scoping `SpatialState` already provides).
2. **Commands** — `kanban-app/src/commands.rs` adds the **happy-path** commands (`spatial_focus`, `spatial_register_focusable`, `spatial_register_zone`, `spatial_unregister_scope`, `spatial_update_rect`, `spatial_navigate`, `spatial_push_layer`, `spatial_pop_layer`). Each command derives `WindowLabel` from `tauri::Window`, locks the registry/state, performs its kernel call, and emits `focus-changed` events to the appropriate windows when the kernel returns a `FocusChangedEvent`.
3. **Handler registration** — `kanban-app/src/main.rs` adds each command name to `tauri::generate_handler![...]`.

This card does NOT cover (left to dependent cards):

- `spatial_register_batch` and the kernel-side fallback during unregister — owned by `01KNS0B3HY...` (this card unblocks it).
- `spatial_drill_in` / `spatial_drill_out` — owned by `01KPZS4RG0...`.
- React `useStableSpatialKeys` / `usePlaceholderRegistration` and the virtualizer integration — owned by `01KNS0B3HY...`.
- Override resolution (rule 0) on `spatial_navigate` — owned by `01KNQY1GQ9...`.

### Crate placement

- `kanban-app/Cargo.toml` — add `swissarmyhammer-focus = { path = "../swissarmyhammer-focus" }` (currently absent).
- `kanban-app/src/state.rs` — `AppState::spatial_state`, `AppState::spatial_registry` fields.
- `kanban-app/src/commands.rs` — eight new `#[tauri::command]` async fns (read-only nav adapters; not state-mutating in the SAH-commands sense, so they bypass `dispatch_command` per the existing top-of-file rule for "transient UI plumbing").
- `kanban-app/src/main.rs` — register handlers.

### Tauri command shapes — newtyped throughout

```rust
#[tauri::command]
pub async fn spatial_focus(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: SpatialKey,
) -> Result<(), String>;

#[tauri::command]
pub async fn spatial_register_focusable(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: SpatialKey,
    moniker: Moniker,
    rect: Rect,
    layer_key: LayerKey,
    parent_zone: Option<SpatialKey>,
    overrides: HashMap<Direction, Option<Moniker>>,
) -> Result<(), String>;

#[tauri::command]
pub async fn spatial_register_zone(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: SpatialKey,
    moniker: Moniker,
    rect: Rect,
    layer_key: LayerKey,
    parent_zone: Option<SpatialKey>,
    overrides: HashMap<Direction, Option<Moniker>>,
) -> Result<(), String>;

#[tauri::command]
pub async fn spatial_unregister_scope(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: SpatialKey,
) -> Result<(), String>;
// IMPORTANT: must call `SpatialState::handle_unregister` BEFORE `SpatialRegistry::unregister_scope`
// so the kernel can read the lost entry's metadata and compute fallback. This ordering is the
// "spatial_unregister_scope reordered" detail referenced in the dynamic-lifecycle card.

#[tauri::command]
pub async fn spatial_update_rect(
    state: State<'_, AppState>,
    key: SpatialKey,
    rect: Rect,
) -> Result<(), String>;

#[tauri::command]
pub async fn spatial_navigate(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: SpatialKey,
    direction: Direction,
) -> Result<(), String>;

#[tauri::command]
pub async fn spatial_push_layer(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: LayerKey,
    name: LayerName,
    parent: Option<LayerKey>,
) -> Result<(), String>;

#[tauri::command]
pub async fn spatial_pop_layer(
    state: State<'_, AppState>,
    key: LayerKey,
) -> Result<(), String>;
```

### Event emission

Each command that may return a `FocusChangedEvent` from the kernel emits it via `window.emit_to(EventTarget::any(), "focus-changed", payload)` (or a label-targeted variant if multi-window scoping requires it). The event shape is the kernel's `FocusChangedEvent` serialized to JSON; the React side already listens for it in `SpatialFocusProvider`.

### Lock ordering

Both `SpatialState` and `SpatialRegistry` are wrapped in `tokio::sync::Mutex` (matching the existing `AppState` pattern). The unregister command holds **both** locks for the duration of the transaction so observers cannot see a half-applied unregister:

```rust
let mut registry = state.spatial_registry.lock().await;
let mut spatial_state = state.spatial_state.lock().await;
let event = spatial_state.handle_unregister(&registry, &key);
registry.unregister_scope(&key);
drop(spatial_state);
drop(registry);
if let Some(event) = event {
    emit_focus_changed(&window, &event)?;
}
```

### Subtasks

- [x] Add `swissarmyhammer-focus` workspace dep to `kanban-app/Cargo.toml`
- [x] Add `spatial_state: Arc<Mutex<SpatialState>>` and `spatial_registry: Arc<Mutex<SpatialRegistry>>` to `AppState` in `kanban-app/src/state.rs`; initialize in `AppState::new()`
- [x] Add eight commands to `kanban-app/src/commands.rs` with the signatures above
- [x] Add a `emit_focus_changed` helper that serializes the kernel's `FocusChangedEvent` and emits via the window handle
- [x] Register all eight commands in `kanban-app/src/main.rs::run_app` `invoke_handler!`
- [x] Confirm `unregister` calls `handle_unregister` before `unregister_scope` (the lock ordering snippet above)
- [x] `cargo build -p kanban-app` succeeds; `cargo clippy -p kanban-app --all-targets -- -D warnings` clean
- [x] React vitest suite: spatial-focus-context tests (which mock `invoke`) still pass — no React changes are made by this card

## Acceptance Criteria

- [x] `kanban-app/Cargo.toml` declares `swissarmyhammer-focus` as a path dependency
- [x] `AppState::spatial_state` and `AppState::spatial_registry` exist; default-initialized; thread-safe via `tokio::sync::Mutex`
- [x] All eight `spatial_*` commands compile, are `#[tauri::command]`, accept newtyped arguments
- [x] `unregister` ordering: `handle_unregister` runs **before** `unregister_scope`
- [x] `main.rs::run_app` registers all eight commands in `invoke_handler!`
- [x] `cargo build` and `cargo clippy --all-targets -- -D warnings` clean for the workspace
- [x] `pnpm vitest run` for `kanban-app/ui` still passes

## Tests

- [x] `kanban-app/src/commands.rs` (or a sibling `tests/spatial_commands.rs`) — `spatial_focus` invokes `SpatialState::focus` under the registry lock and emits `focus-changed`
- [x] `register_focusable` / `register_zone` round-trip through `apply_batch`-style state changes
- [x] `unregister` test: register a leaf, focus it, unregister — assert `handle_unregister` was called BEFORE `unregister_scope` and a `focus-changed` event was emitted
- [x] `navigate` happy path: register two leaves, focus one, `navigate(Down)` returns the other's moniker
- [x] `cargo test -p kanban-app` passes
- [x] `cd kanban-app/ui && npx vitest run` passes

## Workflow

- This is mostly mechanical wiring — the headless kernel already has the API. Stand up the AppState fields, write the eight commands following the same pattern, register them in main.rs, and run `cargo build`. TDD on the Rust side (write a test for `spatial_unregister_scope` ordering first, then make it pass).