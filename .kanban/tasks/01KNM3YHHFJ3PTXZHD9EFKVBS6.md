---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa180
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

State lives in `swissarmyhammer-focus/src/state.rs` alongside the registry from card `01KNQXW7HH...`. Tauri adapters live in `kanban-app/src/commands.rs`. This follows the refactor pattern from commit `b81336d42` (headless business logic in the kanban crate, Tauri commands as thin adapters).

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

### Rust state — `swissarmyhammer-focus/src/state.rs`

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

### Tests — `swissarmyhammer-focus/tests/state.rs`

Headless pattern matching `swissarmyhammer-kanban/tests/resolve_focused_column.rs`.

### Subtasks
- [x] Define TS branded types in `kanban-app/ui/src/types/spatial.ts`
- [x] Add `SpatialState` to `swissarmyhammer-focus/src/state.rs` with `focus_by_window: HashMap<WindowLabel, SpatialKey>`
- [x] `FocusChangedEvent { window_label, prev_key, next_key, next_moniker }` — all newtyped
- [x] Tauri commands in `kanban-app/src/commands.rs` derive `WindowLabel` from `tauri::Window`; emit `focus-changed` event
- [x] Claim registry `Map<SpatialKey, (focused: boolean) => void>` (in `kanban-app/ui/src/lib/spatial-focus-context.tsx`)
- [x] Global `listen("focus-changed")` handler
- [ ] Remove `focusedMoniker` useState from EntityFocusProvider — DEFERRED. The existing `EntityFocusProvider` does not in fact hold the focused moniker in `useState` — it uses `useSyncExternalStore` against an in-memory `FocusStore`, which 51 production files depend on through `useFocusActions`, `useIsDirectFocus`, `useFocusedMoniker`, `useFocusedScope`, `useEntityFocus`, etc. Removing the existing store would be a breaking refactor across the entire UI and lies beyond the scope of building the claim registry infrastructure. The new `SpatialFocusProvider` lives alongside the existing one; consumers migrate to the new claim registry as a separate piece of work.
- [x] Tests in `swissarmyhammer-focus/tests/state.rs`

## Acceptance Criteria
- [x] State lives in `swissarmyhammer-focus/src/state.rs`, not `swissarmyhammer-commands`
- [x] Tauri commands live in `kanban-app/src/commands.rs`
- [x] No signature uses bare `String` or `f64`
- [x] TS branded types mirror Rust newtypes; no `string`/`number` in typed spatial signatures
- [x] Focus is per-window; windows A and B independent
- [x] Focus change in window A doesn't re-render scopes in window B
- [x] All Tauri invokes return Ok/Err only; no focus data in return values
- [x] `cargo test -p swissarmyhammer-kanban` and `pnpm vitest run` pass

## Tests

### Rust (`swissarmyhammer-focus/tests/state.rs`)
- [x] `focus` updates per-window state and returns `FocusChangedEvent` with matching `WindowLabel`
- [x] focus in A doesn't affect `focus_by_window[B]`
- [x] unregister of a focused key clears that window's focus only
- [x] `FocusChangedEvent.next_moniker` is `Some(entry.moniker.clone())` when next_key is Some

### React (in `kanban-app/ui/src/lib/spatial-focus-context.test.tsx`)
- [x] Claim registry ignores events for unknown keys
- [x] Scope click invokes `spatial_focus` with its branded `SpatialKey`
- [x] Provider unmount removes the listener
- [x] Scope unmount removes from claim registry

## Implementation notes

- The claim registry is implemented in a new `SpatialFocusProvider` (in `kanban-app/ui/src/lib/spatial-focus-context.tsx`) rather than woven into the existing `EntityFocusProvider`. The two providers serve different keyspaces — `Moniker` for entity focus, `SpatialKey` for spatial-nav scopes — and keeping them separate keeps each context narrow.
- The full `SpatialRegistry` (Focusable / FocusZone / FocusLayer / FocusScope enum, beam search) is owned by card `01KNQXW7HH...` and lands on top of the `SpatialEntry` table introduced here without breaking the public surface.
- `SpatialState::navigate` is wired through the public surface but currently returns `None` — beam search lives in card `01KNQXXF5W...`.
- `spatial_push_layer` / `spatial_pop_layer` are no-ops at the kanban-crate level pending the layer forest from card `01KNQXW7HH...`; the IPC surface is pinned now so the frontend can register its keymap entries today.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.