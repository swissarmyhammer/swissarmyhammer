---
assignees:
- claude-code
position_column: todo
position_ordinal: '9980'
title: Move focusedMoniker from React state to Rust UIState for backend-testable focus management
---
## What

Move the `focusedMoniker` (the currently focused entity moniker, e.g. `task:01ABC`) from React `useState` in `EntityFocusProvider` to a **ref + pub/sub** pattern, and store it in Rust `UIState` for backend-testable focus management. This eliminates the current O(all-FocusScopes) re-render on every focus change, replacing it with exactly 2 re-renders (the scope losing focus + the scope gaining focus).

### Current state — the problem

- `EntityFocusProvider` (`kanban-app/ui/src/lib/entity-focus-context.tsx:79`) owns `focusedMoniker` as `useState<string | null>(null)`.
- Every component consuming `useEntityFocus()` re-renders when focus changes — that's every mounted `FocusScope` (dozens on a board).
- On every focus change, `setFocus` builds the scope chain and dispatches `ui.setFocus` which stores the chain in `UIState.scope_chain` (`swissarmyhammer-commands/src/ui_state.rs:150`), but the actual focused moniker is not stored in Rust.
- Rust commands can't know what's focused without the frontend passing it as an arg.

### Architecture — ref + pub/sub (no context re-renders)

**EntityFocusProvider** stores focus in a **ref**, not state. It exposes `subscribe`, `getFocused`, and `setFocus` via context. Changing focus never triggers a React context re-render.

```
setFocus("task:02")
  → focusRef.current = "task:02"
  → notify all subscribers: (prev="task:01", next="task:02")
      → FocusScope moniker="task:01": prev===me → setIsFocused(false) → re-render
      → FocusScope moniker="task:02": next===me → setIsFocused(true) → re-render
      → FocusScope moniker="task:03": neither → no re-render
      → FocusScope moniker="column:todo": neither → no re-render
      → ... (all other scopes: no re-render)
  → dispatch("ui.setFocus", {focused_moniker, scope_chain}) → Rust stores both
```

Exactly 2 FocusScopes re-render per focus change, regardless of board size.

### Rust side

**`swissarmyhammer-commands/src/ui_state.rs`**:
- Add `focused_moniker: Option<String>` to `UIStateInner` (transient, `#[serde(skip)]`).
- Update `set_scope_chain` to also accept and store `focused_moniker`. Or add a new `set_focus(moniker, scope_chain)` method that sets both atomically.
- Add `fn focused_moniker(&self) -> Option<String>` reader.
- Include `focused_moniker` in `debug_snapshot` output.

**`swissarmyhammer-kanban/src/commands/ui_commands.rs`**:
- Update `SetFocusCmd::execute` to read `focused_moniker` from args and pass it to UIState.

### Frontend side

**`kanban-app/ui/src/lib/entity-focus-context.tsx`** — the main refactor:

1. Replace `useState<string | null>(null)` with `useRef<string | null>(null)` for `focusedMoniker`.
2. Add a `listenersRef = useRef(new Set<(prev, next) => void>())` for the subscriber set.
3. `setFocus(moniker)` reads prev from ref, writes next to ref, notifies all listeners with `(prev, next)`, dispatches `ui.setFocus` with `{focused_moniker, scope_chain}` to Rust. No `setState`.
4. Context value exposes `subscribe(cb)`, `getFocused()`, and `setFocus(moniker)`. The context object itself is stable (memoized once) — it never changes, so consumers never re-render from context changes.
5. `broadcastNavCommand` reads from `focusRef.current` (already does this via `focusedMonikerRef`).

**`kanban-app/ui/src/components/focus-scope.tsx`**:

1. Each FocusScope subscribes via `useEffect(() => subscribe((prev, next) => { if (prev === moniker || next === moniker) setIsFocused(next === moniker); }), [moniker])`.
2. Replace `const isDirectFocus = focusedMoniker === moniker` (which reads from context state) with local `const [isFocused, setIsFocused] = useState(false)` driven by the subscription.
3. On mount, sync: `setIsFocused(getFocused() === moniker)`.

**`kanban-app/ui/src/lib/entity-focus-context.tsx` — `useIsFocused(moniker)`**:

1. Same subscription pattern: `useState(false)` + subscribe + check `prev === moniker || next === moniker`.
2. Only re-renders when the boolean flips for this specific moniker.

**`kanban-app/ui/src/lib/entity-focus-context.tsx` — `FocusedScopeContext`**:

1. Currently derived from `focusedMoniker` state on every render. With the ref approach, update it inside the `setFocus` function by looking up the registry. Since `FocusedScopeContext` is used by `useDispatchCommand`, it needs to be a context value — but it can be updated via `setState` only when the focused scope actually changes (which is every focus change). This is acceptable because `useDispatchCommand` consumers don't re-render from it — they read it lazily during dispatch.

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` — add `focused_moniker` field, reader, update `set_scope_chain`
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — update `SetFocusCmd` to store `focused_moniker`
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — ref + pub/sub refactor, update `useIsFocused`, update context shape
- `kanban-app/ui/src/components/focus-scope.tsx` — subscribe to focus changes, local `isFocused` state

## Acceptance Criteria
- [ ] `EntityFocusProvider` stores focus in a ref, not React state — changing focus triggers zero context re-renders
- [ ] Each `FocusScope` subscribes to focus changes and only re-renders when it gains or loses focus (exactly 2 re-renders per focus change)
- [ ] `useIsFocused(moniker)` only re-renders when the boolean result flips for that specific moniker
- [ ] `UIStateInner` has `focused_moniker: Option<String>` (transient, `#[serde(skip)]`)
- [ ] `ui.setFocus` command stores both `focused_moniker` and `scope_chain` in UIState
- [ ] `UIState::focused_moniker()` returns the currently focused moniker
- [ ] Existing keyboard navigation works identically (no regressions in claim predicates, nav commands, or visual focus indicators)

## Tests
- [ ] `swissarmyhammer-commands/src/ui_state.rs` — Unit test: set_focus stores focused_moniker, focused_moniker() reads it back, clearing focus clears both moniker and scope_chain
- [ ] `swissarmyhammer-kanban/src/commands/mod.rs` — Update SetFocusCmd test: verify focused_moniker stored when args include it
- [ ] `kanban-app/ui/src/lib/entity-focus-context.test.tsx` — Test: subscriber receives (prev, next) on focus change; subscriber with moniker matching neither prev nor next does not trigger setState; getFocused() returns current value synchronously
- [ ] `kanban-app/ui/src/components/focus-scope.test.tsx` (new or existing) — Test: FocusScope re-renders only when it gains/loses focus, not when unrelated focus changes occur
- [ ] Run `cargo test -p swissarmyhammer-commands` and `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.