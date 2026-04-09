---
assignees:
- claude-code
depends_on:
- 01KNM3YHHFJ3PTXZHD9EFKVBS6
position_column: todo
position_ordinal: a180
project: spatial-nav
title: 'Spatial registry: Rust-side spatial state, React-side rect measurement'
---
## What

Add spatial awareness to the focus system with a clear frontend/backend split:

- **React** measures DOM rects and reports them to Rust. React owns `FocusScope` (measures its element) and `FocusLayer` (declares a layer boundary).
- **Rust** owns the spatial registry, layer stack, and navigation algorithm. All navigation logic is backend-testable with synthetic rect data — no DOM needed.

### Keying model

Two separate key systems:

- **Moniker** (`"task:01ABC"`, `"field:task:01ABC.title"`) — entity identity. Used for command scope chains, `setFocus`, backend dispatch. Unchanged. Non-entity UI elements use a `"ui:region.action"` convention (e.g., `"ui:toolbar.newTask"`, `"ui:tabbar.board1"`).
- **Spatial key** (ULID) — unique per FocusScope/FocusLayer *instance*. Generated once per mount via `useRef(ulid())`. Stable across re-renders, new on remount. This is the primary key in the Rust spatial registry.

Why separate: the same moniker can appear in multiple locations (task card on board + mention pill in inspector). Each gets its own spatial key, own rect, possibly different layer. Rust stores `HashMap<SpatialKey, SpatialEntry>` where `SpatialEntry = { moniker, rect, layer_key }`. `navigate()` returns the winning entry's **moniker** (what `setFocus` needs).

FocusLayer also gets a ULID spatial key so the layer stack handles multiple instances of the same named layer (e.g., two inspector panels in two windows).

### Architecture

```
React (measure + report)              Rust (track + navigate)
─────────────────────────              ──────────────────────
FocusScope mounts
  spatialKey = useRef(ulid())
  → ResizeObserver fires
  → invoke: spatial.register           → registry.insert(spatial_key, { moniker, rect, layer_key })
    { key, moniker, rect, layer_key }

FocusScope unmounts
  → invoke: spatial.unregister         → registry.remove(spatial_key)
    { key }

FocusLayer mounts
  layerKey = useRef(ulid())
  → invoke: spatial.push_layer         → layer_stack.push({ key, name })
    { key, name }

FocusLayer unmounts
  → invoke: spatial.pop_layer          → layer_stack.remove(key)
    { key }

nav.up key pressed
  → invoke: spatial.navigate           → find_target(direction, focused_moniker)
    { direction, focused_moniker }       filter to active layer, score by geometry
  ← returns target moniker             ← winning entry's moniker
  → setFocus(target)
```

### Rust side

**`SpatialEntry`** struct:
- `spatial_key: String` (ULID from React)
- `moniker: String`
- `rect: Rect { x: f64, y: f64, width: f64, height: f64 }`
- `layer_key: String` (ULID of the FocusLayer this scope lives in)

**`SpatialRegistry`** struct:
- `entries: HashMap<String, SpatialEntry>` — keyed by spatial_key
- `register(key, moniker, rect, layer_key)` / `unregister(key)` / `update_rect(key, rect)`

**`LayerStack`** — `Vec<LayerEntry>` where `LayerEntry = { key: String, name: String }`:
- `push(key, name)` / `remove(key)` / `active() -> Option<&LayerEntry>` (topmost)
- `remove` by key (not pop) — handles out-of-order unmount if an inner layer unmounts before an outer one

**Tauri commands**:
- `spatial_register(key, moniker, x, y, w, h, layer_key)`
- `spatial_unregister(key)`
- `spatial_push_layer(key, name)`
- `spatial_pop_layer(key)`
- `spatial_navigate(direction, focused_moniker) -> Option<String>` — returns target moniker

### React side

**`FocusLayer`** component (`kanban-app/ui/src/components/focus-layer.tsx`):
- Props: `name: string`, `children`
- `const layerKey = useRef(ulid()).current`
- Provides `layerKey` via React context (so FocusScope can read it)
- On mount: `invoke("spatial_push_layer", { key: layerKey, name })`
- On unmount: `invoke("spatial_pop_layer", { key: layerKey })`

**`FocusScope`** changes (`kanban-app/ui/src/components/focus-scope.tsx`):
- `const spatialKey = useRef(ulid()).current`
- Read `layerKey` from FocusLayer context (throw if missing)
- Add `ResizeObserver` on the wrapper div
- On mount/resize: `invoke("spatial_register", { key: spatialKey, moniker, ...rect, layer_key: layerKey })`
- On unmount: `invoke("spatial_unregister", { key: spatialKey })`

**`app-shell.tsx`**:
- Wrap app root in `<FocusLayer name="window">`

**`entity-focus-context.tsx`** — `broadcastNavCommand`:
1. Evaluate `claimWhen` predicates (backward compat, temporary)
2. If no claim: `const target = await invoke("spatial_navigate", { direction, focused_moniker })`
3. If target: `setFocus(target)`

### Design decisions
- **ULID per instance**: `useRef(ulid())` — stable across re-renders, unique per mount. No global counter, no collision risk.
- **Layer removal by key, not pop**: Layer stack supports arbitrary removal order. If inspector A opens, then dialog B opens on top, then A closes — `remove(A.key)` works correctly. B is still on top.
- **Moniker for focus, key for registry**: `navigate()` returns monikers because `setFocus` and command dispatch use monikers. The spatial key is internal plumbing.
- **Non-entity monikers**: UI chrome uses `"ui:region.action"` convention. These participate in spatial nav and can register commands in their FocusScope, but don't go through entity command dispatch.

### Subtasks
- [ ] Define `Rect`, `SpatialEntry`, `SpatialRegistry`, `LayerEntry`, `LayerStack` in Rust
- [ ] Implement registry and layer stack with key-based operations
- [ ] Add Tauri commands wiring React to Rust
- [ ] Create FocusLayer component with ULID key + Tauri invokes
- [ ] Update FocusScope with ULID spatial key + ResizeObserver + Tauri invokes

## Acceptance Criteria
- [ ] Each FocusScope mount generates a unique ULID spatial key via `useRef`
- [ ] Each FocusLayer mount generates a unique ULID layer key via `useRef`
- [ ] Spatial registry keyed by spatial key — same moniker in two locations = two entries
- [ ] Layer stack supports arbitrary removal order (not just pop)
- [ ] `navigate()` returns target moniker (not spatial key)
- [ ] Root `<FocusLayer name="window">` wraps the app
- [ ] Existing focus behavior unchanged
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests
- [ ] `Rust unit tests` — registry: register two entries with same moniker, different keys — both stored; unregister one, other remains
- [ ] `Rust unit tests` — layer stack: push A, push B, remove A — B is still active; remove B — stack empty
- [ ] `kanban-app/ui/src/components/focus-layer.test.tsx` — ULID generated on mount, stable across re-renders, new on remount
- [ ] `kanban-app/ui/src/components/focus-scope.test.tsx` — spatial key generated, registered with layer key from context
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.