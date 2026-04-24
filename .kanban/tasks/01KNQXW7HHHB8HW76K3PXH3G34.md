---
assignees:
- claude-code
depends_on:
- 01KNM3YHHFJ3PTXZHD9EFKVBS6
position_column: todo
position_ordinal: a180
project: spatial-nav
title: 'Rust kernel: Focusable, FocusZone, FocusLayer, FocusScope types + SpatialRegistry'
---
## What

Define the Rust kernel types for spatial focus. These types **peer** with React components of the same names (see the React-primitives and FocusScope-refactor cards). UI is authoritative for structure — React declares the components and registers them via Tauri commands. Rust owns all computation (beam search, fallback, layer ops) and emits events.

### Crate placement

Following the pattern established in commit `b81336d42` (move GUI-crate logic into headless-testable kanban crate), **this lives in `swissarmyhammer-kanban`**, not `swissarmyhammer-commands`. Rationale: spatial nav is business logic that must be testable without Tauri or jsdom; `swissarmyhammer-commands` stays Tier 0 (no app state, no windowing concepts).

Specifically, organize as a submodule alongside the existing `focus.rs`:

```
swissarmyhammer-kanban/src/focus/
  mod.rs          (re-exports; existing resolve_focused_column migrates here)
  column.rs       (existing resolve_focused_column logic)
  types.rs        (newtypes: WindowLabel, SpatialKey, LayerKey, Moniker, LayerName, Pixels, Rect, Direction)
  scope.rs        (Focusable, FocusZone, FocusScope enum)
  layer.rs        (FocusLayer struct)
  registry.rs     (SpatialRegistry with scope + layer ops)
  state.rs        (SpatialState with focus_by_window, event emission)
  navigate.rs     (beam search algorithm — separate card 01KNQXXF5W...)
```

Migration of `focus.rs` → `focus/column.rs` is part of this card's scope; keep the public re-export path (`swissarmyhammer_kanban::focus::resolve_focused_column`) stable.

### The four peer types

| Role                | React component     | Rust type             |
|---------------------|---------------------|-----------------------|
| Leaf focusable point| `Focusable`         | `struct Focusable`    |
| Navigable container | `FocusZone`         | `struct FocusZone`    |
| Modal layer boundary| `FocusLayer`        | `struct FocusLayer`   |
| Entity-aware wrapper| `FocusScope`        | `enum FocusScope`     |

On Rust: `FocusScope` is the sum type `Focusable | FocusZone` stored in the registry per `SpatialKey`. On React: `FocusScope` is the entity-aware wrapper that composes `<Focusable>` or `<FocusZone>`.

### Newtype discipline — use `swissarmyhammer_common::define_id!`

The project already has a canonical newtype macro: `define_id!` from `swissarmyhammer-common`. It provides `#[serde(transparent)]`, `Display`, `AsRef<str>`, `From<&str>`/`From<String>`, `Deref`, `Borrow<str>`, `FromStr`, `PartialEq<str>`, plus `new()` (fresh ULID), `from_string()`, `as_str()`. All existing ID types (`TaskId`, `ColumnId`, `TagId`, etc.) use it.

Define the string-valued newtypes with this macro:

```rust
use swissarmyhammer_common::define_id;

define_id!(WindowLabel, "Tauri window label — which window a scope/layer lives in");
define_id!(SpatialKey, "ULID per FocusScope/FocusZone/Focusable instance");
define_id!(LayerKey,   "ULID per FocusLayer instance");
define_id!(Moniker,    "Entity focus identity: \"task:01ABC\", \"ui:toolbar.new\"");
define_id!(LayerName,  "Layer role: \"window\", \"inspector\", \"dialog\", \"palette\"");
```

`Pixels` is numeric, not string — hand-rolled:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pixels(pub f64);

// Arithmetic so beam/score math doesn't drop to f64:
impl std::ops::Add for Pixels { /* ... */ }
impl std::ops::Sub for Pixels { /* ... */ }
impl std::ops::Mul<f64> for Pixels { /* ... */ }
impl std::ops::Div<f64> for Pixels { /* ... */ }
```

### Rect

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: Pixels,
    pub y: Pixels,
    pub width: Pixels,
    pub height: Pixels,
}

impl Rect {
    pub fn top(&self)    -> Pixels { self.y }
    pub fn left(&self)   -> Pixels { self.x }
    pub fn bottom(&self) -> Pixels { self.y + self.height }
    pub fn right(&self)  -> Pixels { self.x + self.width }
}
```

### The four types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Focusable {
    pub key: SpatialKey,
    pub moniker: Moniker,
    pub rect: Rect,
    pub layer_key: LayerKey,
    pub parent_zone: Option<SpatialKey>,
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusZone {
    pub key: SpatialKey,
    pub moniker: Moniker,
    pub rect: Rect,
    pub layer_key: LayerKey,
    pub parent_zone: Option<SpatialKey>,
    pub last_focused: Option<SpatialKey>,
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusLayer {
    pub key: LayerKey,
    pub name: LayerName,
    pub parent: Option<LayerKey>,
    pub window_label: WindowLabel,
    pub last_focused: Option<SpatialKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FocusScope {
    Focusable(Focusable),
    Zone(FocusZone),
}

impl FocusScope {
    pub fn key(&self) -> &SpatialKey;
    pub fn moniker(&self) -> &Moniker;
    pub fn rect(&self) -> &Rect;
    pub fn layer_key(&self) -> &LayerKey;
    pub fn parent_zone(&self) -> Option<&SpatialKey>;
    pub fn overrides(&self) -> &HashMap<Direction, Option<Moniker>>;
    pub fn is_zone(&self) -> bool;
    pub fn is_focusable(&self) -> bool;
    pub fn as_zone(&self) -> Option<&FocusZone>;
    pub fn as_zone_mut(&mut self) -> Option<&mut FocusZone>;
}
```

### Registry

```rust
pub struct SpatialRegistry {
    scopes: HashMap<SpatialKey, FocusScope>,
    layers: HashMap<LayerKey, FocusLayer>,
}

impl SpatialRegistry {
    pub fn register_focusable(&mut self, f: Focusable);
    pub fn register_zone(&mut self, z: FocusZone);
    pub fn unregister_scope(&mut self, key: &SpatialKey);
    pub fn update_rect(&mut self, key: &SpatialKey, rect: Rect);
    pub fn scope(&self, key: &SpatialKey) -> Option<&FocusScope>;
    pub fn children_of_zone(&self, zone_key: &SpatialKey) -> impl Iterator<Item = &FocusScope>;
    pub fn ancestor_zones(&self, key: &SpatialKey) -> Vec<&FocusZone>;

    pub fn push_layer(&mut self, l: FocusLayer);
    pub fn remove_layer(&mut self, key: &LayerKey);
    pub fn layer(&self, key: &LayerKey) -> Option<&FocusLayer>;
    pub fn children_of_layer(&self, key: &LayerKey) -> Vec<&FocusLayer>;
    pub fn root_for_window(&self, label: &WindowLabel) -> Option<&FocusLayer>;
    pub fn ancestors_of_layer(&self, key: &LayerKey) -> Vec<&FocusLayer>;

    pub fn scopes_in_layer(&self, key: &LayerKey) -> impl Iterator<Item = &FocusScope>;
}
```

### Tauri commands — in `kanban-app/src/commands.rs`

The headless registry lives in `swissarmyhammer-kanban`. Tauri adapters live in `kanban-app/src/commands.rs` (same file as `list_entities`, `get_entity`, etc.), each deriving `window_label` from the `tauri::Window` parameter and delegating to the kanban crate:

```rust
#[tauri::command]
pub async fn spatial_register_focusable(
    window: tauri::Window,
    state: State<'_, AppState>,
    key: SpatialKey,
    moniker: Moniker,
    rect: Rect,
    layer_key: LayerKey,
    parent_zone: Option<SpatialKey>,
    overrides: HashMap<Direction, Option<Moniker>>,
) -> Result<(), String>;

// Likewise: spatial_register_zone, spatial_unregister_scope, spatial_update_rect,
//           spatial_push_layer, spatial_pop_layer, spatial_focus, spatial_navigate,
//           spatial_drill_in, spatial_drill_out
```

### Tests — `swissarmyhammer-kanban/tests/focus_registry.rs`

Follow the pattern of `swissarmyhammer-kanban/tests/resolve_focused_column.rs` and `dynamic_sources_headless.rs` — pure-Rust, no Tauri, no jsdom. Constructs synthetic `Rect`/`SpatialKey`/`Moniker` values with `::from_string` / `Pixels(..)`.

### Design decisions

- **`swissarmyhammer-kanban` not `swissarmyhammer-commands`**: matches the refactor pattern — business logic over Tier 0.
- **`define_id!` macro reuse**: consistent with `TaskId`/`ColumnId`/`TagId`; avoids hand-rolled boilerplate.
- **`focus/` submodule**: groups all focus concerns (column resolver + spatial registry) together.
- **Four peer types, not a `kind` field on one struct**: zone-only fields (`last_focused`) are type-checked.
- **`FocusScope` as a Rust enum**: one map, pattern matching.

### Subtasks
- [ ] Create `swissarmyhammer-kanban/src/focus/` directory; move existing `focus.rs` → `focus/column.rs`; `focus/mod.rs` re-exports to keep the public path stable
- [ ] `focus/types.rs`: define `WindowLabel`, `SpatialKey`, `LayerKey`, `Moniker`, `LayerName` via `define_id!`; hand-roll `Pixels` + arithmetic; define `Rect`, `Direction`
- [ ] `focus/scope.rs`: define `Focusable`, `FocusZone`, `FocusScope` enum with serde attrs + helper methods
- [ ] `focus/layer.rs`: define `FocusLayer`
- [ ] `focus/registry.rs`: implement `SpatialRegistry` with scope + layer ops
- [ ] Forest ops (`children_of_layer`, `root_for_window`, `ancestors_of_layer`)
- [ ] Zone tree ops (`children_of_zone`, `ancestor_zones`)
- [ ] Add Tauri commands in `kanban-app/src/commands.rs` with fully-typed signatures; each derives `WindowLabel` from `tauri::Window`
- [ ] Tests in `swissarmyhammer-kanban/tests/focus_registry.rs` following the headless pattern

## Acceptance Criteria
- [ ] Location is `swissarmyhammer-kanban/src/focus/` — NOT `swissarmyhammer-commands`
- [ ] String-valued newtypes use `define_id!` from `swissarmyhammer-common` — NOT hand-rolled
- [ ] Every identity or measurement parameter on every public type / command signature uses a newtype; no bare `String` or `f64`
- [ ] `Focusable`, `FocusZone`, `FocusLayer` exist as distinct structs; `FocusScope` is an enum over `Focusable | Zone`
- [ ] `Pixels` supports `+` / `-` / `*`/f64 / `/`/f64 without `.0` access
- [ ] Existing `resolve_focused_column` still accessible at `swissarmyhammer_kanban::focus::resolve_focused_column`
- [ ] Tauri commands live in `kanban-app/src/commands.rs`, derive `WindowLabel` from `tauri::Window`
- [ ] Tests run as `cargo test -p swissarmyhammer-kanban`; no Tauri or jsdom required

## Tests (pure Rust, in `swissarmyhammer-kanban/tests/focus_registry.rs`)
- [ ] Each newtype JSON-round-trips as a bare primitive (`#[serde(transparent)]` from `define_id!`)
- [ ] `Pixels` arithmetic is type-preserving
- [ ] `FocusScope::Focusable(_)` and `FocusScope::Zone(_)` round-trip with `"kind"` tag
- [ ] Registry: register a Focusable + a FocusZone; `scope(key)` returns the right variant
- [ ] Registry: `children_of_zone` direct children only
- [ ] Registry: `ancestor_zones` walks `parent_zone` up to layer root
- [ ] Registry: `children_of_layer` filtered by parent, not cross-window
- [ ] Registry: `ancestors_of_layer` walks `layer.parent`
- [ ] Registry: `scopes_in_layer` returns both Focusables and Zones by `layer_key`
- [ ] Registry: 2 windows + 2 inspector layers + 1 dialog = 5 layers, 2 roots, correct chains

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.