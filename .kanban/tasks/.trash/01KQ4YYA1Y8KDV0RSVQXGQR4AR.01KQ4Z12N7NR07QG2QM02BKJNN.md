---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9680
project: spatial-nav
title: 'Tauri spatial-nav adapter: AppState plumbing + spatial_* commands'
---
## What

Wire the existing `swissarmyhammer-focus` kernel into the kanban-app Tauri layer. The Rust kernel (registry, state, navigate strategy, drill, batch register) is fully built in `swissarmyhammer-focus` and the React-side primitives (`<Focusable>`, `<FocusZone>`, `<FocusLayer>`) plus the `SpatialFocusProvider` are fully built — they all invoke `spatial_*` Tauri commands. None of those Tauri commands exist yet because no card has owned the AppState plumbing.

This task adds the AppState plumbing and the full Tauri-command suite that React expects, in one place, before the per-feature cards (drill, batch register, etc.) can wire their own commands on top.

## Crate placement

- `kanban-app/Cargo.toml` adds `swissarmyhammer-focus = { workspace = true }` (workspace dep already declared by 01KQ2E7RPBPJ8T8KZX39N2SZ0A).
- `kanban-app/src/state.rs` adds a `SpatialState`-bearing field on `AppState` behind a `tokio::sync::Mutex` — `spatial: Mutex<SpatialState>`. The mutex is fine here: spatial commands are short, do not await DB I/O, and serialising them keeps the registry / focus_by_window invariants simple.
- `kanban-app/src/commands.rs` adds the Tauri-command surface listed below.
- `kanban-app/src/main.rs` registers each new command in `tauri::generate_handler!`.

## Tauri commands to add

Every command derives `WindowLabel` from the `tauri::Window` parameter, takes a `State<AppState>`, locks the spatial mutex, delegates to the `swissarmyhammer-focus` kernel, and emits `focus-changed` to all windows when the kernel returns a `FocusChangedEvent`.

```rust
// Registration
#[tauri::command] pub async fn spatial_register_focusable(window, state, key, moniker, rect, layer_key, parent_zone, overrides) -> Result<(), String>;
#[tauri::command] pub async fn spatial_register_zone(window, state, key, moniker, rect, layer_key, parent_zone, overrides) -> Result<(), String>;
#[tauri::command] pub async fn spatial_register_batch(window, state, entries: Vec<RegisterEntry>) -> Result<(), String>;
#[tauri::command] pub async fn spatial_unregister_scope(window, state, key) -> Result<(), String>;
#[tauri::command] pub async fn spatial_update_rect(window, state, key, rect) -> Result<(), String>;

// Focus / nav
#[tauri::command] pub async fn spatial_focus(window, state, key) -> Result<(), String>;
#[tauri::command] pub async fn spatial_navigate(window, state, key, direction) -> Result<(), String>;

// Drill (this card just provides the surface; the drill logic ships in 01KPZS4RG0JWSFZ8KQ1PM88WRH)
#[tauri::command] pub async fn spatial_drill_in(window, state, key) -> Result<Option<Moniker>, String>;
#[tauri::command] pub async fn spatial_drill_out(window, state, key) -> Result<Option<Moniker>, String>;

// Layers
#[tauri::command] pub async fn spatial_push_layer(window, state, key, name, parent) -> Result<(), String>;
#[tauri::command] pub async fn spatial_pop_layer(window, state, key) -> Result<(), String>;
```

Per-command behavior — each one matches the kernel method of the same shape; they are thin lockers + emitters. Wire-format tags (`kind` discriminator on `RegisterEntry`, etc.) are inherited from the focus crate's `serde` config.

## Subtasks
- [ ] Add `swissarmyhammer-focus` to `kanban-app/Cargo.toml`
- [ ] Add `spatial: Mutex<SpatialState>` to `AppState` (constructed in `with_ui_state`, default-empty)
- [ ] Add the `spatial_*` Tauri commands listed above as thin adapters
- [ ] Register every new command in `tauri::generate_handler![ ... ]` in `main.rs`
- [ ] Wire `focus-changed` emit at the adapter layer — the kernel returns `Option<FocusChangedEvent>`; on `Some`, emit to all windows
- [ ] Smoke test: `cargo build -p kanban-app` succeeds; `cargo test -p kanban-app` still passes
- [ ] Smoke test: existing `cd kanban-app/ui && pnpm vitest run` continues to pass (the React-side mocks of `invoke` were already in place — adding the real backend should not regress them)

## Acceptance Criteria
- [ ] Every `spatial_*` Tauri command React already invokes is registered and reachable
- [ ] Command signatures use newtypes from `swissarmyhammer_focus::*`; no bare `String` or `f64` on the surface
- [ ] `AppState::spatial` is the single source of truth for the spatial registry / focus state
- [ ] `focus-changed` is emitted to **all** webview windows whenever `SpatialState` returns a `FocusChangedEvent` (frontend filters by window identity through scope registration, not at the event layer)
- [ ] `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings` and `pnpm vitest run` all pass with zero warnings
- [ ] No existing Tauri command surface is changed — this is purely additive

## Tests
- [ ] Unit test in `commands.rs` (or a sibling `spatial_commands.rs`) that constructs a fresh `AppState`, drives the adapter through a register / focus / unregister sequence, and asserts the kernel state matches expectations
- [ ] Confirm `cargo test -p kanban-app` and `cd kanban-app/ui && pnpm vitest run` are green

## Workflow
- This unblocks 01KPZS4RG0JWSFZ8KQ1PM88WRH (drill), 01KNS0B3HYNXDFGV3ZMN6JCK1E (dynamic lifecycle / batch register), and any future spatial-nav card that ships a Tauri adapter
- Use `/tdd` — write a failing adapter test, then make it pass

## Origin

Spun out 2026-04-26 by the `/implement` run on 01KPZS4RG0JWSFZ8KQ1PM88WRH after the user pointed out that the plan was out of order — the spatial-nav epic does not yet have a card that owns the Tauri-side AppState plumbing, so per-feature cards (drill, batch, etc.) cannot land their commands cleanly. The previously-completed cards (`01KNM3YHHFJ3...`, `01KNQXXF5W...`, `01KNQXW7HH...`) listed Tauri-command wiring as a subtask but each deferred it to "downstream cards"; that pattern produced a gap. This card closes the gap.