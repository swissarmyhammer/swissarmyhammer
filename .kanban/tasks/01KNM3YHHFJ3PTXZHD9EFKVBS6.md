---
assignees:
- claude-code
position_column: todo
position_ordinal: '9980'
project: spatial-nav
title: 'Focus claim registry: key-based, event-driven, Rust owns state'
---
## What

Replace the current `focusedMoniker` React state with an event-driven **focus claim registry** keyed by spatial key (ULID). Rust owns all focus state. React is a dumb renderer that responds to events.

### Core principle

- **Rust owns focus state.** `focused_key: Option<String>` lives in Rust. All focus changes go through Rust.
- **React sends commands, Rust emits events.** No return values. Every Tauri invoke returns Ok/Err. Focus changes are communicated via a `"focus-changed"` event.
- **Focus identity = key (ULID).** Not moniker. The claim registry is `Map<key, callback>`. Monikers are metadata for command dispatch, separate from focus mechanics.

### API surface

```
React → Rust (all return Ok/Err, nothing else):
  spatial_register(key, moniker, rect, layer_key)   — FocusScope mount/resize
  spatial_unregister(key)                            — FocusScope unmount
  spatial_navigate(direction)                        — keyboard nav
  spatial_focus(key)                                 — click / programmatic
  spatial_push_layer(key, name)                      — FocusLayer mount
  spatial_pop_layer(key)                             — FocusLayer unmount

Rust → React (Tauri event):
  "focus-changed" { prev_key: Option<String>, next_key: Option<String> }
```

`spatial_navigate` reads the current `focused_key` from Rust's own state, runs the beam test + scoring, updates `focused_key` to the winner, and emits `"focus-changed"`. No return value needed.

`spatial_focus(key)` sets `focused_key` directly and emits `"focus-changed"`. Used for clicks and programmatic focus.

### React side

**Global event listener** (in EntityFocusProvider):

```typescript
listen("focus-changed", ({ prev_key, next_key }) => {
  if (prev_key) claimRegistry.get(prev_key)?.(false);  // one re-render
  if (next_key) claimRegistry.get(next_key)?.(true);    // one re-render
});
```

**FocusScope**:
- Generates ULID key: `const key = useRef(ulid()).current`
- Registers in claim registry: `registry.set(key, (focused) => setIsFocused(focused))`
- On mount/resize: `invoke("spatial_register", { key, moniker, ...rect, layer_key })`
- On unmount: `invoke("spatial_unregister", { key })`
- On click: `invoke("spatial_focus", { key })` — fire and forget
- Local `const [isFocused, setIsFocused] = useState(false)` driven by claim callback

**No focusedMoniker state in React.** No ref. No pub/sub. React doesn't track who's focused — it just responds to events from Rust.

### Moniker is separate

When a FocusScope receives `claim(true)`, it knows its own moniker. It can separately dispatch `ui.setFocus` with moniker + scope chain for command dispatch purposes. But this is the command system's concern, not the focus system's.

### Rust side

**`SpatialState`** (or extend UIState):
- `focused_key: Option<String>`
- On `spatial_navigate`: resolve direction → update `focused_key` → emit event
- On `spatial_focus(key)`: update `focused_key` → emit event
- On `spatial_unregister(key)`: if key == focused_key, clear focus → emit event

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` or new `spatial_state.rs` — `focused_key`, event emission
- `swissarmyhammer-kanban/src/commands/` — Tauri commands for spatial_*
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — claim registry, global event listener, remove useState
- `kanban-app/ui/src/components/focus-scope.tsx` — key generation, claim registration, fire-and-forget invokes

### Subtasks
- [ ] Add claim registry `Map<string, (focused: boolean) => void>` to EntityFocusProvider
- [ ] Add global `listen("focus-changed")` handler that claims/unclaims by key
- [ ] Update FocusScope: ULID key, register claim, fire-and-forget invokes
- [ ] Add `focused_key` to Rust state with event emission on change
- [ ] Remove `focusedMoniker` useState from EntityFocusProvider

## Acceptance Criteria
- [ ] Focus change triggers exactly 2 FocusScope re-renders via event callback
- [ ] All Tauri invokes return Ok/Err only — no focus data in return values
- [ ] Focus changes flow through Rust event: click → Rust → event → React claim
- [ ] Keyboard nav: React invokes `spatial_navigate(direction)` → Rust emits event → React claims
- [ ] No `focusedMoniker` state in React — Rust owns focus state
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests
- [ ] `entity-focus-context.test.tsx` — claim registry: register, event fires, correct callbacks called
- [ ] `entity-focus-context.test.tsx` — unregistered key in event is a no-op
- [ ] `focus-scope.test.tsx` — FocusScope registers claim on mount, unregisters on unmount
- [ ] `focus-scope.test.tsx` — click invokes spatial_focus with key, not moniker
- [ ] `Rust unit tests` — spatial_focus updates focused_key and emits event
- [ ] `Rust unit tests` — spatial_navigate updates focused_key and emits event
- [ ] `Rust unit tests` — spatial_unregister of focused key clears focus and emits event
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.