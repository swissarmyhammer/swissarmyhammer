---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: done
position_ordinal: ffffffffffffffffffffffd980
project: spatial-nav
title: 'Spatial navigation algorithm in Rust: nearest-neighbor by direction'
---
## What

Implement the spatial navigation algorithm in Rust as a pure function on `SpatialRegistry`. Informed by prior art from Android FocusFinder, W3C CSS Spatial Navigation, UWP XYFocus, and Norigin Spatial Navigation.

### Subtasks
- [x] Implement beam test: filter candidates into in-beam and out-of-beam sets
- [x] Implement Android-style scoring: `13 * major² + minor²`
- [x] Implement container-first search: parent scope siblings first, then full layer
- [x] Implement edge commands (First, Last, RowStart, RowEnd)
- [x] Add focus memory to LayerStack entries

## Acceptance Criteria
- [x] In-beam candidates always preferred over out-of-beam
- [x] Aligned candidates preferred over closer-but-diagonal (13:1 ratio)
- [x] Container-first: nav stays in parent scope if siblings exist
- [x] Container fallback: no sibling → expands to full layer
- [x] Focus memory: layer pop restores last-focused key
- [x] All 8 directions correct
- [x] Hard layer boundary — entries in other layers excluded
- [x] `cargo test` passes (38 spatial tests)

## Implementation

### `spatial_nav.rs` — pure algorithm module
- `Direction` enum with `ParseDirectionError` typed error
- `find_target` + `container_first_search` with documented caller contract (must exclude source from candidates)
- 11 unit tests

### `spatial_state.rs` — extended state machine
- `SpatialStateInner::save_focus_memory()` — extracted helper (no duplication between focus/navigate)
- `SpatialState::navigate()` — layer-filtered container-first search
- Focus memory: `focus()` and `navigate()` both call `save_focus_memory`; `remove_layer()` restores
- 4 integration tests

### Tauri commands
- `spatial_navigate` — real implementation
- `spatial_register` — accepts `parent_scope`
- `spatial_remove_layer` — renamed from `spatial_pop_layer` for clarity

## Review Findings (2026-04-15 19:45)

### Warnings
- [x] `swissarmyhammer-commands/src/spatial_state.rs` — **Duplicated focus-memory save logic.** Extracted `SpatialStateInner::save_focus_memory()` helper, called from both `focus()` and `navigate()`.
- [x] `swissarmyhammer-commands/src/spatial_nav.rs` — **Missing doc contract on candidates.** Added doc comments on both `find_target` and `container_first_search` stating caller must exclude source.

### Nits
- [x] `kanban-app/src/spatial.rs` — **`spatial_pop_layer` misleadingly named.** Renamed to `spatial_remove_layer` in Tauri command, main.rs registration, React FocusLayer, and tests.
- [x] `swissarmyhammer-commands/src/spatial_nav.rs` — **`FromStr` error type is `String`.** Added `ParseDirectionError` struct with `Display` and `Error` impls."

## Review Findings (2026-04-15 20:12)

### Nits
- [ ] `swissarmyhammer-commands/src/spatial_state.rs` — **Stale test name after rename.** Test function `layer_pop_restores_last_focused` still uses "pop" terminology but exercises `remove_layer()`. Rename to `layer_remove_restores_last_focused` for consistency with finding 3.