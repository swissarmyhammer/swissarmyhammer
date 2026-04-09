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
  spatial_navigate(key, direction)                   — keyboard nav (from key, in direction)
  spatial_focus(key)                                 — click / programmatic
  spatial_push_layer(key, name)                      — FocusLayer mount
  spatial_pop_layer(key)                             — FocusLayer unmount

Rust → React (Tauri event):
  "focus-changed" { prev_key: Option<String>, next_key: Option<String> }
```

### React side

**Global event listener** (in EntityFocusProvider):

```typescript
listen("focus-changed", ({ prev_key, next_key }) => {
  if (prev_key) claimRegistry.get(prev_key)?.(false);
  if (next_key) claimRegistry.get(next_key)?.(true);
});
```

**FocusScope**:
- Generates ULID key: `const key = useRef(ulid()).current`
- Registers in claim registry: `registry.set(key, setIsFocused)`
- On click: `invoke("spatial_focus", { key })` — fire and forget
- Local `const [isFocused, setIsFocused] = useState(false)` driven by claim callback

**No focusedMoniker state in React.** React doesn't track who's focused — it just responds to events from Rust.

### Moniker is separate

When a FocusScope receives `claim(true)`, it knows its own moniker. It can separately dispatch `ui.setFocus` with moniker + scope chain for command dispatch purposes. But this is the command system's concern, not the focus system's.

### Rust side

**`SpatialState`** (or extend UIState):
- `focused_key: Option<String>`
- On `spatial_navigate(key, direction)`: resolve → update `focused_key` → emit event
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
- [ ] Keyboard nav: React invokes `spatial_navigate(key, direction)` → Rust emits event → React claims
- [ ] No `focusedMoniker` state in React — Rust owns focus state
- [ ] Event listener cleaned up on EntityFocusProvider unmount
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests

### Rust unit tests (`swissarmyhammer-commands/src/spatial_state.rs` or similar)

```rust
#[test]
fn spatial_focus_updates_focused_key_and_emits_event() {
    // Given: empty state
    // When: spatial_focus("key-abc")
    // Then: state.focused_key == Some("key-abc")
    // And: emitted event == FocusChanged { prev_key: None, next_key: Some("key-abc") }
}

#[test]
fn spatial_focus_emits_prev_and_next() {
    // Given: focused_key == Some("key-1")
    // When: spatial_focus("key-2")
    // Then: event == FocusChanged { prev_key: Some("key-1"), next_key: Some("key-2") }
}

#[test]
fn spatial_unregister_focused_key_clears_focus() {
    // Given: focused_key == Some("key-1"), key-1 is registered
    // When: spatial_unregister("key-1")
    // Then: focused_key == None
    // And: event == FocusChanged { prev_key: Some("key-1"), next_key: None }
}

#[test]
fn spatial_unregister_non_focused_key_no_event() {
    // Given: focused_key == Some("key-1"), key-2 is registered
    // When: spatial_unregister("key-2")
    // Then: focused_key still Some("key-1"), no event emitted
}
```

### React unit tests (`kanban-app/ui/src/lib/entity-focus-context.test.tsx`)

Mock setup:
```typescript
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_event, callback) => {
    // Store callback so tests can fire events manually
    (listen as any).__callback = callback;
    return Promise.resolve(() => {}); // unsub function
  }),
}));
```

```
test: "claim registry calls previous callback with false and next with true on focus-changed event"
  setup: render EntityFocusProvider with two FocusScopes (key-A, key-B)
  act: fire focus-changed event { prev_key: null, next_key: "key-A" }
  assert: FocusScope A has data-focused attribute, B does not
  act: fire focus-changed event { prev_key: "key-A", next_key: "key-B" }
  assert: FocusScope A no longer data-focused, FocusScope B has data-focused

test: "unregistered key in focus-changed event is a no-op"
  setup: render EntityFocusProvider with one FocusScope (key-A)
  act: fire focus-changed event { prev_key: "nonexistent", next_key: "key-A" }
  assert: no error thrown, FocusScope A has data-focused

test: "FocusScope click invokes spatial_focus with its key"
  setup: render FocusScope inside EntityFocusProvider + FocusLayer
  act: click the FocusScope element
  assert: invoke called with ("spatial_focus", { key: <the ULID> })

test: "EntityFocusProvider unmount cleans up event listener"
  setup: render then unmount EntityFocusProvider
  assert: the unsub function returned by listen() was called

test: "FocusScope unmount removes from claim registry"
  setup: render FocusScope, capture its key
  act: unmount FocusScope, then fire focus-changed { next_key: <captured key> }
  assert: no error, no state update (callback was unregistered)
```

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.