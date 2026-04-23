---
assignees:
- claude-code
depends_on:
- 01KNM3YHHFJ3PTXZHD9EFKVBS6
position_column: done
position_ordinal: ffffffffffffffffffffffd880
project: spatial-nav
title: 'Spatial registry: Rust-side spatial state, React-side rect measurement'
---
## What

Add spatial awareness to the focus system with a clear frontend/backend split:

- **React** measures DOM rects and reports them to Rust. React owns `FocusScope` (measures its element) and `FocusLayer` (declares a layer boundary).
- **Rust** owns the spatial registry, layer stack, and navigation algorithm. All navigation logic is backend-testable with synthetic rect data — no DOM needed.

### Subtasks
- [x] Define `Rect`, `SpatialEntry`, `SpatialRegistry`, `LayerEntry`, `LayerStack` in Rust
- [x] Implement registry and layer stack with key-based operations
- [x] Add Tauri commands wiring React to Rust
- [x] Create FocusLayer component with ULID key + Tauri invokes
- [x] Update FocusScope with ULID spatial key + ResizeObserver + Tauri invokes

## Acceptance Criteria
- [x] Each FocusScope mount generates a unique ULID spatial key via `useRef`
- [x] Each FocusLayer mount generates a unique ULID layer key via `useRef`
- [x] Spatial registry keyed by spatial key — same moniker in two locations = two entries
- [x] Layer stack supports arbitrary removal order (not just pop)
- [x] `navigate()` returns target moniker (not spatial key) — stub returns None, ready for card 2
- [x] Root `<FocusLayer name=\"window\">` wraps the app in AppShell
- [x] Existing focus behavior unchanged
- [x] `cargo test` passes (23 spatial_state tests), `pnpm vitest run` passes (1111 tests)

## Implementation Summary

### Rust (`swissarmyhammer-commands/src/spatial_state.rs`)
- `Rect { x, y, width, height: f64 }` with `right()` / `bottom()`
- `SpatialEntry { key, moniker, rect, layer_key }` — full struct
- `LayerStack` — `Vec<LayerEntry>` with `push(key, name)`, `remove(key)`, `active()` (topmost)
- `SpatialState.register(key, moniker, rect, layer_key)` / `update_rect(key, rect)` / `push_layer` / `remove_layer` / `active_layer`
- 23 unit tests passing (focus, registry, layer stack, rect)

### Tauri commands (`kanban-app/src/spatial.rs`)
- `spatial_register(key, moniker, x, y, w, h, layer_key)` — full rect + layer key
- `spatial_push_layer(key, name)` / `spatial_pop_layer(key)` — real implementations (not stubs)

### React
- `FocusLayer` component (`focus-layer.tsx`) — ULID key, context provider, Tauri push/pop on mount/unmount
- `FocusScope` — reads layer key from `FocusLayerContext`, ResizeObserver measures DOM rect, reports to Rust on mount/resize
- `useFocusLayerKey()` returns `null` when no FocusLayer ancestor (graceful degradation for tests)
- `useSpatialClaim` skips Rust registration when no layer key (tests work without FocusLayer)
- `AppShell` wrapped in `<FocusLayer name=\"window\">`

### Tests
- 6 new FocusLayer tests (push/pop invokes, key stability, remount, context access)
- Updated focus-scope and entity-focus-context tests with FocusLayer wrappers where appropriate

## Review Findings (2026-04-15 17:45)

### Nits
- [ ] `kanban-app/src/spatial.rs:112` — `spatial_pop_layer` command name implies stack pop semantics but performs removal by key. Consider renaming to `spatial_remove_layer` to match the Rust-side `remove_layer` method it delegates to.
- [ ] `swissarmyhammer-commands/src/spatial_state.rs:59` — `SpatialEntry` is missing `PartialEq` derive. Adding it would simplify test assertions and is free since `Rect` already derives `PartialEq` and all other fields are `String`.
- [ ] `swissarmyhammer-commands/src/spatial_state.rs:30` — `Rect` is missing `Default` derive. A zero-origin zero-size rect is a natural default and would be useful for tests and builder patterns in the upcoming navigation card.
- [ ] `kanban-app/ui/src/components/focus-layer.tsx:13` — Local variable `ref` shadows the JSX reserved word `ref`. While functionally harmless (it's a local binding, not a JSX attribute), consider `keyRef` for clarity.