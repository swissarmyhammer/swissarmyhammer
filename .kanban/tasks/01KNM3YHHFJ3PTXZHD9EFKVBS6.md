---
assignees:
- claude-code
position_column: todo
position_ordinal: '9980'
project: spatial-nav
title: 'Unified focus claim registry: Rust decides, React does O(1) lookup by spatial key'
---
## What

Replace the current `focusedMoniker` React state + pub/sub proposal with a unified **focus claim registry** that serves both spatial navigation and all other focus changes. Rust owns all focus decisions. React provides a hook-based registry keyed by spatial key (ULID) for O(1) focus delivery.

### Architecture

```
Any focus trigger (click, keyboard, programmatic)
  → Rust: decides target spatial_key
  → React: claimRegistry.get(spatial_key).claim()
  → That one FocusScope re-renders (isFocused = true)
  → Previous focused scope: claim(false) → re-renders (isFocused = false)
  → All other scopes: untouched
```

### Focus Claim Registry

A `Map<string, ClaimCallback>` at the app root, provided via React context. Each FocusScope registers on mount with its ULID spatial key.

```typescript
type ClaimCallback = (focused: boolean) => void;

// App root
const registryRef = useRef(new Map<string, ClaimCallback>());

// In each FocusScope
const spatialKey = useRef(ulid()).current;
const [isFocused, setIsFocused] = useState(false);

useEffect(() => {
  registry.set(spatialKey, (focused) => setIsFocused(focused));
  return () => registry.delete(spatialKey);
}, [spatialKey]);
```

### Focus change flow

When focus changes (from any source — click, spatial nav, programmatic):

1. Look up the previous spatial key → call `claim(false)` (one scope re-renders, hides focus bar)
2. Look up the new spatial key → call `claim(true)` (one scope re-renders, shows focus bar)
3. Store the new spatial key + moniker in Rust
4. Dispatch `ui.setFocus` with moniker + scope chain to backend

**Exactly 2 re-renders per focus change, regardless of board size.**

### How this unifies with spatial nav

Spatial navigation (`invoke("spatial_navigate")`) returns a **spatial key**, not a moniker. React uses the claim registry to deliver focus to the target. The same registry handles:
- Click → FocusScope's click handler calls a focus function with its own spatial key
- Keyboard nav → Rust returns target spatial key, React claims it
- Programmatic → `setFocus(moniker)` resolves to spatial key via a reverse lookup, then claims

### Moniker ↔ spatial key resolution

Rust maintains a bidirectional index:
- `spatial_key → SpatialEntry { moniker, rect, layer_key, ... }` (primary registry)
- `moniker → Vec<spatial_key>` (reverse index for `setFocus(moniker)` — returns entries in the active layer)

When React calls `setFocus(moniker)` (e.g., from a click), it invokes Rust to resolve the moniker to the correct spatial key in the active layer, then claims via the registry.

### What changes from the original card

The original card proposed ref + pub/sub with subscriber callbacks matching on moniker. This revision:
- **Keys on spatial key (ULID), not moniker** — handles duplicate monikers (same entity in two places)
- **Registry is a simple Map, not a Set of filter callbacks** — O(1) lookup, no iteration
- **Claim callback is `(focused: boolean) => void`** — direct, no prev/next comparison
- **Rust owns focus state** — `focused_spatial_key` and `focused_moniker` both stored in Rust

### Rust side

**`swissarmyhammer-commands/src/ui_state.rs`**:
- Add `focused_spatial_key: Option<String>` (transient, `#[serde(skip)]`)
- Add `focused_moniker: Option<String>` (transient, `#[serde(skip)]`)
- Both set atomically by `set_focus(spatial_key, moniker, scope_chain)`

### React side

**`kanban-app/ui/src/lib/entity-focus-context.tsx`**:
- Replace `useState<string | null>` for `focusedMoniker` with refs for both `focusedSpatialKey` and `focusedMoniker`
- Add `claimRegistryRef = useRef(new Map<string, ClaimCallback>())`
- `registerClaim(spatialKey, callback)` / `unregisterClaim(spatialKey)` — called by FocusScope
- `claimFocus(spatialKey)` — unclaims previous, claims new, updates refs, dispatches to Rust
- Context value is stable (memoized once) — `{ registerClaim, unregisterClaim, claimFocus, getFocused }`. Never changes, no consumer re-renders.

**`kanban-app/ui/src/components/focus-scope.tsx`**:
- `useRef(ulid())` for spatial key
- Register in claim registry on mount, unregister on unmount
- Local `useState(false)` for `isFocused`, driven by claim callback
- Click handler calls `claimFocus(spatialKey)` instead of `setFocus(moniker)`

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` — add `focused_spatial_key`, `focused_moniker`
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — update `SetFocusCmd`
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — claim registry, stable context
- `kanban-app/ui/src/components/focus-scope.tsx` — register/unregister claim, local isFocused state

### Subtasks
- [ ] Add claim registry (Map<string, ClaimCallback>) to EntityFocusProvider
- [ ] Implement `claimFocus(spatialKey)` — unclaim prev, claim new, update Rust
- [ ] Update FocusScope to register in claim registry with ULID spatial key
- [ ] Update FocusScope to use local `isFocused` state driven by claim callback
- [ ] Add `focused_spatial_key` + `focused_moniker` to Rust UIState

## Acceptance Criteria
- [ ] Focus change triggers exactly 2 FocusScope re-renders (losing + gaining)
- [ ] Claim registry is O(1) lookup by spatial key
- [ ] Context value is stable — no consumer re-renders from context changes
- [ ] Click, keyboard nav, and programmatic focus all use the same claim path
- [ ] Same moniker in two locations (board + inspector) handled correctly via spatial key
- [ ] Rust stores `focused_spatial_key` and `focused_moniker`
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests
- [ ] `entity-focus-context.test.tsx` — claim registry: register, claim, unclaim, unregister
- [ ] `entity-focus-context.test.tsx` — claimFocus triggers exactly 2 callbacks (prev + next)
- [ ] `entity-focus-context.test.tsx` — unregistered spatial key is a no-op (no crash)
- [ ] `focus-scope.test.tsx` — FocusScope registers on mount, unregisters on unmount
- [ ] `focus-scope.test.tsx` — only the focused FocusScope has isFocused=true
- [ ] `Rust unit tests` — UIState stores and retrieves focused_spatial_key + focused_moniker
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.