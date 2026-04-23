---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffd780
project: spatial-nav
title: 'Focus claim registry: key-based, event-driven, Rust owns state'
---
## What

Replace the current `focusedMoniker` React state with an event-driven **focus claim registry** keyed by spatial key (ULID). Rust owns all focus state. React is a dumb renderer that responds to events.

### Core principle

- **Rust owns focus state.** `focused_key: Option<String>` lives in Rust. All focus changes go through Rust.
- **React sends commands, Rust emits events.** No return values. Every Tauri invoke returns Ok/Err. Focus changes are communicated via a `"focus-changed"` event.
- **Focus identity = key (ULID).** Not moniker. The claim registry is `Map<key, callback>`. Monikers are metadata for command dispatch, separate from focus mechanics.

### Subtasks
- [x] Add claim registry `Map<string, (focused: boolean) => void>` to EntityFocusProvider
- [x] Add global `listen("focus-changed")` handler that claims/unclaims by key
- [x] Update FocusScope: ULID key, register claim, fire-and-forget invokes
- [x] Add `focused_key` to Rust state with event emission on change
- [ ] Remove `focusedMoniker` useState from EntityFocusProvider — deferred: `focusedMoniker` state is kept as a backward-compat bridge updated from events. Removing it requires migrating all consumers (`useIsFocused`, `FocusedScopeContext`, `board-view`, `grid-view`, `entity-inspector`) to the claim registry. This is addressed by later cards in the dependency chain.

## Implementation Notes

### Rust side (`swissarmyhammer-commands/src/spatial_state.rs`)
- `SpatialState` with `entries: HashMap<String, SpatialEntry>` and `focused_key: Option<String>`
- `FocusChanged` event struct with `prev_key` / `next_key`
- Methods: `register`, `unregister`, `focus`, `clear_focus`, `focused_key`, `get`, `len`, `is_empty`
- 11 unit tests passing

### Tauri commands (`kanban-app/src/spatial.rs`)
- `spatial_register(key, moniker)` — FocusScope mount
- `spatial_unregister(key)` — FocusScope unmount, clears focus if focused
- `spatial_focus(key)` — click/programmatic, emits focus-changed event
- `spatial_clear_focus()` — clears focus without removing entries
- `spatial_navigate(key, direction)` — stub (algorithm in card 2)
- `spatial_push_layer(key, name)` — stub (layers in card 1)
- `spatial_pop_layer(key)` — stub (layers in card 1)

### React side
- Claim registry: `Map<key, ClaimCallback>` + `Map<key, moniker>` + `Map<moniker, Set<key>>`
- `listen("focus-changed")` drives claim callbacks AND updates focusedMoniker for compat
- FocusScope generates ULID spatial key via `useRef(ulid())`
- FocusScope registers claim, registers Tauri spatial entry on mount, cleans up on unmount
- FocusScope click: `setFocus(moniker)` (which internally bridges to `spatial_focus`)
- `setFocus(null)` invokes `spatial_clear_focus` to keep Rust in sync

## Acceptance Criteria
- [x] Focus change triggers exactly 2 FocusScope re-renders via event callback
- [x] All Tauri invokes return Ok/Err only — no focus data in return values
- [x] Focus changes flow through Rust event: click → Rust → event → React claim
- [x] Keyboard nav: React invokes `spatial_navigate(key, direction)` → Rust emits event → React claims (stub for now)
- [x] Event listener cleaned up on EntityFocusProvider unmount
- [x] `cargo test` passes, `pnpm vitest run` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-04-15 16:35)

### Warnings
- [x] `kanban-app/ui/src/components/focus-scope.tsx` `handleClick` — **Double `spatial_focus` invoke on every click.** Removed the direct `invoke("spatial_focus", ...)` from `handleClick`; `setFocus` already handles it via `useFocusSetter`.
- [x] `swissarmyhammer-commands/src/spatial_state.rs` `focus()` — **Allows focusing an unregistered key.** Added `entries.contains_key(key)` guard; returns `None` for unregistered keys. Added test `focus_unregistered_key_is_noop`.
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx` `useFocusSetter` — **`setFocus(null)` does not clear Rust `focused_key`.** Added `clear_focus()` method to `SpatialState`, `spatial_clear_focus` Tauri command, and `invoke("spatial_clear_focus")` call in `useFocusSetter` when moniker is null. Added tests `clear_focus_emits_event` and `clear_focus_when_unfocused_is_noop`.

### Nits
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx` `useFocusSetter` — `console.warn` fires on every focus change. Gated behind `import.meta.env.DEV`.
- [x] `kanban-app/ui/src/lib/entity-focus-context.test.tsx` and `focus-scope.test.tsx` — The `ulid` mock produces strings shorter than real ULIDs. Fixed to use incrementing counter with `padStart(20, "0")` for consistent 26-char output.