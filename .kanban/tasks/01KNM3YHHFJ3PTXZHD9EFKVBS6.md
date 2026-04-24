---
assignees:
- claude-code
position_column: todo
position_ordinal: '9980'
project: spatial-nav
title: 'Focus claim registry: per-window, event-driven, Rust owns state (newtype-only signatures)'
---
## What

Replace the current `focusedMoniker` React state with an event-driven **focus claim registry** keyed by `SpatialKey`. Rust owns all focus state — tracked **per window** because the app is multi-window (see `UIState.windows`, per-window `inspector_stack`). Every signature uses newtypes, not bare primitives.

### Core principle

- **Rust owns focus state.** React is a dumb renderer that responds to events.
- **Focus is per window.** Each Tauri window has its own focused element.
- **Focus identity = `SpatialKey` (ULID).** Not `Moniker`. The claim registry is `Map<SpatialKey, callback>`.

### Crate placement

State lives in `swissarmyhammer-kanban/src/focus/state.rs` alongside the registry from card `01KNQXW7HH...`. Tauri adapters live in `kanban-app/src/commands.rs`. This follows the refactor pattern from commit `b81336d42` (headless business logic in the kanban crate, Tauri commands as thin adapters).

### Newtypes — use `define_id!`

All string-valued newtypes are defined via `swissarmyhammer_common::define_id!` in `focus/types.rs` (see card `01KNQXW7HH...` for full definitions). Referenced here for self-containment:

```rust
define_id!(WindowLabel, "Tauri window label");
define_id!(SpatialKey, "ULID per scope mount");
define_id!(LayerKey, "ULID per layer mount");
define_id!(Moniker, "Entity focus identity");
```

No bare `String` appears in any signature below.

### API surface

```rust
// Tauri commands in kanban-app/src/commands.rs; all derive WindowLabel from tauri::Window.
async fn spatial_register_focusable(window: tauri::Window, key: SpatialKey, /* ... */) -> Result<(), String>;
async fn spatial_register_zone(window: tauri::Window, key: SpatialKey, /* ... */) -> Result<(), String>;
async fn spatial_unregister_scope(window: tauri::Window, key: SpatialKey) -> Result<(), String>;
async fn spatial_focus(window: tauri::Window, key: SpatialKey) -> Result<(), String>;
async fn spatial_navigate(window: tauri::Window, key: SpatialKey, direction: Direction) -> Result<(), String>;
async fn spatial_push_layer(window: tauri::Window, key: LayerKey, name: LayerName, parent: Option<LayerKey>) -> Result<(), String>;
async fn spatial_pop_layer(window: tauri::Window, key: LayerKey) -> Result<(), String>;
```

```rust
// Emitted from Rust to React
#[derive(Serialize)]
pub struct FocusChangedEvent {
    pub window_label: WindowLabel,
    pub prev_key: Option<SpatialKey>,
    pub next_key: Option<SpatialKey>,
    pub next_moniker: Option<Moniker>,
}
```

### Rust state — `swissarmyhammer-kanban/src/focus/state.rs`

```rust
pub struct SpatialState {
    registry: SpatialRegistry,
    focus_by_window: HashMap<WindowLabel, SpatialKey>,
}

impl SpatialState {
    pub fn focus(&mut self, window: WindowLabel, key: SpatialKey) -> Option<FocusChangedEvent>;
    pub fn navigate(&mut self, window: WindowLabel, key: SpatialKey, direction: Direction) -> Option<FocusChangedEvent>;
    pub fn handle_unregister(&mut self, window: WindowLabel, key: SpatialKey) -> Option<FocusChangedEvent>;
}
```

Methods return `Option<FocusChangedEvent>` rather than side-effecting so tests don't need Tauri mocking — the Tauri adapter in `kanban-app/src/commands.rs` is responsible for emitting the event.

### React side — branded types

Parity with Rust newtypes via TypeScript branded strings. Details live in the React-primitives card (`01KPZWY4B7...`). The `types/spatial.ts` file exports `WindowLabel`, `SpatialKey`, `LayerKey`, `Moniker`, `LayerName`, `Pixels` as branded types with brand helpers.

**Global event listener** (in EntityFocusProvider):

```typescript
listen<FocusChangedPayload>("focus-changed", ({ payload }) => {
  if (payload.prev_key) claimRegistry.get(payload.prev_key)?.(false);
  if (payload.next_key) claimRegistry.get(payload.next_key)?.(true);
});
```

Each Tauri window has its own React tree and its own claim registry, so a `focus-changed` event for another window's key is a no-op here.

### Tests — `swissarmyhammer-kanban/tests/focus_state.rs`

Headless pattern matching `swissarmyhammer-kanban/tests/resolve_focused_column.rs`.

### Subtasks
- [ ] Define TS branded types in `kanban-app/ui/src/types/spatial.ts`
- [ ] Add `SpatialState` to `swissarmyhammer-kanban/src/focus/state.rs` with `focus_by_window: HashMap<WindowLabel, SpatialKey>`
- [ ] `FocusChangedEvent { window_label, prev_key, next_key, next_moniker }` — all newtyped
- [ ] Tauri commands in `kanban-app/src/commands.rs` derive `WindowLabel` from `tauri::Window`; emit `focus-changed` event
- [ ] Claim registry `Map<SpatialKey, (focused: boolean) => void>` in EntityFocusProvider
- [ ] Global `listen("focus-changed")` handler
- [ ] Remove `focusedMoniker` useState from EntityFocusProvider
- [ ] Tests in `swissarmyhammer-kanban/tests/focus_state.rs`

## Acceptance Criteria
- [ ] State lives in `swissarmyhammer-kanban/src/focus/state.rs`, not `swissarmyhammer-commands`
- [ ] Tauri commands live in `kanban-app/src/commands.rs`
- [ ] No signature uses bare `String` or `f64`
- [ ] TS branded types mirror Rust newtypes; no `string`/`number` in typed spatial signatures
- [ ] Focus is per-window; windows A and B independent
- [ ] Focus change in window A doesn't re-render scopes in window B
- [ ] All Tauri invokes return Ok/Err only; no focus data in return values
- [ ] `cargo test -p swissarmyhammer-kanban` and `pnpm vitest run` pass

## Tests

### Rust (`swissarmyhammer-kanban/tests/focus_state.rs`)
- [ ] `focus` updates per-window state and returns `FocusChangedEvent` with matching `WindowLabel`
- [ ] focus in A doesn't affect `focus_by_window[B]`
- [ ] unregister of a focused key clears that window's focus only
- [ ] `FocusChangedEvent.next_moniker` is `Some(entry.moniker.clone())` when next_key is Some

### React
- [ ] Claim registry ignores events for unknown keys
- [ ] Scope click invokes `spatial_focus` with its branded `SpatialKey`
- [ ] Provider unmount removes the listener
- [ ] Scope unmount removes from claim registry

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.