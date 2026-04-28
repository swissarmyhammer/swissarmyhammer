---
assignees:
- claude-code
depends_on:
- 01KNM3YHHFJ3PTXZHD9EFKVBS6
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa480
project: spatial-nav
title: 'Rust kernel: Focusable, FocusZone, FocusLayer, FocusScope types + SpatialRegistry'
---
## What

Define the Rust kernel types for spatial focus inside the new `swissarmyhammer-focus` crate. These types **peer** with React components of the same names (see the React-primitives and FocusScope-refactor cards). UI is authoritative for structure — React declares the components and registers them via Tauri commands. Rust owns all computation (beam search, fallback, layer ops) and emits events.

## Update — three peers, not four (kanban card 01KQ5PP55SAAVJ0V3HDJ1DGNBY)

This card was authored when the spatial-nav kernel was planned to expose **four peers**: `Focusable` (leaf), `FocusZone` (container), `FocusLayer` (modal boundary), and a composite `FocusScope` enum that the registry stored per `SpatialKey`. After implementing it that way, the composite-vs-primitive split smeared `showFocusBar`, click handling, and indicator rendering across two layers and turned out to be the systemic root of why visible focus was missing on most components.

Card `01KQ5PP55SAAVJ0V3HDJ1DGNBY` collapsed the model into **three peers**:

| Concept | Role | React | Rust |
|---|---|---|---|
| Layer | Modal boundary, hard nav stop | `<FocusLayer>` | `FocusLayer` |
| Zone | Navigable container | `<FocusZone>` | `FocusZone` |
| Scope | Leaf — shows focus, takes clicks, navigates | `<FocusScope>` | `FocusScope` |

The leaf struct that used to be named `Focusable` is now `FocusScope`. The public `FocusScope` enum is gone — the registry stores leaves and zones via an internal discriminator that is not part of the public API. Consumers iterate the registry through `leaves_in_layer`, `zones_in_layer`, `leaves_iter`, and `zones_iter`, which yield the typed structs directly. The Tauri command `spatial_register_focusable` was renamed to `spatial_register_scope`. `<Focusable>` is a transitional re-export of `<FocusScope>` until the per-component cards finish migrating their imports; the file gets deleted in card `01KQ5PSMYE3Q60SV8270S6K819`.

The terminology section below was originally written against the four-peer model. Read the **three-peer rewrite** below as the live definition; the four-peer wording is preserved underneath for historical context.

### Crate placement

**Lives in `swissarmyhammer-focus`** (created by card `01KQ2E7RPBPJ8T8KZX39N2SZ0A`). Spatial focus is generic — opaque `Moniker` strings, abstract `Rect`s, `WindowLabel`s. No kanban concepts, no Tauri, no domain types. Putting it in `swissarmyhammer-kanban` was wrong; the dedicated crate makes it reusable, easier to test, and keeps the kanban crate focused on board logic.

```
swissarmyhammer-focus/src/
  lib.rs            (module declarations)
  types.rs          ← THIS CARD: newtypes (WindowLabel, SpatialKey, LayerKey, Moniker, LayerName, Pixels) + Rect + Direction
  scope.rs          ← THIS CARD: FocusScope (leaf struct) + FocusZone struct + internal RegisteredScope enum
  layer.rs          ← THIS CARD: FocusLayer struct
  registry.rs       ← THIS CARD: SpatialRegistry with scope + layer ops; ChildScope enum for variant-aware children
  state.rs          (SpatialState — card 01KNM3YHHFJ3...)
  navigate.rs       (BeamNavStrategy impl — card 01KNQXXF5W...)
  observer.rs       (FocusEventSink impls — card 01KQ2E7RPBPJ8...)
```

### The three peer types (live)

| Role                | React component     | Rust type                                    |
|---------------------|---------------------|----------------------------------------------|
| Leaf focus point    | `<FocusScope>`      | `swissarmyhammer_focus::FocusScope`          |
| Navigable container | `<FocusZone>`       | `swissarmyhammer_focus::FocusZone`           |
| Modal layer boundary| `<FocusLayer>`      | `swissarmyhammer_focus::FocusLayer`          |

There is no public sum-type enum that conflates leaves and zones. The registry stores them via an internal `RegisteredScope` enum that is not exported.

### Terminology — canonical definitions (three-peer rewrite)

**Layer** (`FocusLayer`)
- A **hard modal boundary**. Spatial nav, fallback resolution, and zone tree walks **never cross a layer**.
- Layers form a **forest**: each Tauri window has its own root layer; inspector / dialog / palette overlays are stacked child layers under their parent layer.
- Examples: `window` (root, one per Tauri webview), `inspector` (one per window when any inspector is open), `dialog`, `palette`.
- A layer is *not* itself focusable — you don't navigate "to" a layer; you navigate within the active focus's layer.
- Identified by `LayerKey` (ULID per mount).

**Zone** (`FocusZone`)
- A **soft navigable container** within a layer. Zones group leaves; the beam search prefers within-zone candidates first (rule 1) before falling back across zones (rule 2).
- Zones form a **tree within a layer**, rooted at the layer root (a top-level zone or directly at `parent_zone = None`).
- Examples: board container, column, card, inspector panel, field row, nav bar, toolbar group, perspective bar, view container.
- Each zone has its own `last_focused: Option<SpatialKey>` for drill-out / fallback memory.
- A zone *is* focusable — you can drill out to it, then nav between sibling zones (zone-level beam search).
- Identified by `SpatialKey` (ULID per mount).

**Scope** (`FocusScope`)
- A **leaf focusable point** — atomic, no children, no zone-level features.
- Examples: task title text, status pill, tag pill, mention pill, button, menu item, breadcrumb item.
- Identified by `SpatialKey`.
- The Rust struct `FocusScope` is the leaf. The React `<FocusScope>` is the same — a leaf primitive that registers via `spatial_register_scope`, subscribes to per-key focus claims, renders the visible focus indicator, takes click events, and pushes the entity-aware `CommandScope` / right-click / double-click chrome.

The umbrella term "**any registered focus point**" still applies (a leaf or a zone, but never a layer); the internal `RegisteredScope` enum captures it for in-crate iteration. There is no public enum to navigate, by design.

### Disambiguation: `CommandScope` is a separate concept

The existing kanban codebase has `CommandScope` (in `kanban-app/ui/src/lib/command-scope.tsx`) — that is the **command-dispatch** boundary used to resolve which scope handles a dispatched command (like `ui.inspect`). It is *not* the same as `FocusScope`. The React `<FocusScope>` component creates **both** a spatial entry (in `swissarmyhammer-focus`) **and** a `CommandScope` (in the kanban app). The two systems share the moniker but are otherwise independent.

`swissarmyhammer-focus` itself has no concept of `CommandScope` — it's pure spatial-nav, generic across consumers.

### Newtype discipline — use `swissarmyhammer_common::define_id!`

```rust
use swissarmyhammer_common::define_id;

define_id!(WindowLabel, "Tauri window label — which window a scope/layer lives in");
define_id!(SpatialKey, "ULID per FocusLayer/FocusZone/FocusScope instance");
define_id!(LayerKey,   "ULID per FocusLayer instance");
define_id!(Moniker,    "Entity focus identity: \"task:01ABC\", \"ui:toolbar.new\"");
define_id!(LayerName,  "Layer role: \"window\", \"inspector\", \"dialog\", \"palette\"");
```

`Pixels` is numeric — hand-rolled with arithmetic so beam math stays type-safe:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pixels(pub f64);

impl std::ops::Add for Pixels { /* ... */ }
impl std::ops::Sub for Pixels { /* ... */ }
impl std::ops::Mul<f64> for Pixels { /* ... */ }
impl std::ops::Div<f64> for Pixels { /* ... */ }
```

### Rect

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: Pixels, pub y: Pixels, pub width: Pixels, pub height: Pixels,
}
impl Rect {
    pub fn top(&self) -> Pixels { self.y }
    pub fn left(&self) -> Pixels { self.x }
    pub fn bottom(&self) -> Pixels { self.y + self.height }
    pub fn right(&self) -> Pixels { self.x + self.width }
}
```

### The three peer types (live shape)

```rust
/// Leaf — Rust peer of `<FocusScope>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusScope {
    pub key: SpatialKey,
    pub moniker: Moniker,
    pub rect: Rect,
    pub layer_key: LayerKey,
    pub parent_zone: Option<SpatialKey>,
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

/// Container — Rust peer of `<FocusZone>`.
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

/// Modal layer — Rust peer of `<FocusLayer>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusLayer {
    pub key: LayerKey,
    pub name: LayerName,
    pub parent: Option<LayerKey>,
    pub window_label: WindowLabel,
    pub last_focused: Option<SpatialKey>,
}
```

### Registry

```rust
pub struct SpatialRegistry { /* opaque */ }

impl SpatialRegistry {
    pub fn register_scope(&mut self, f: FocusScope);          // was register_focusable
    pub fn register_zone(&mut self, z: FocusZone);
    pub fn unregister_scope(&mut self, key: &SpatialKey);
    pub fn update_rect(&mut self, key: &SpatialKey, rect: Rect);

    pub fn scope(&self, key: &SpatialKey) -> Option<&FocusScope>;     // leaf only
    pub fn zone(&self, key: &SpatialKey) -> Option<&FocusZone>;       // zone only
    pub fn is_registered(&self, key: &SpatialKey) -> bool;            // variant-blind
    pub fn leaves_iter(&self) -> impl Iterator<Item = &FocusScope>;
    pub fn zones_iter(&self) -> impl Iterator<Item = &FocusZone>;
    pub fn leaves_in_layer(&self, key: &LayerKey) -> impl Iterator<Item = &FocusScope>;
    pub fn zones_in_layer(&self, key: &LayerKey) -> impl Iterator<Item = &FocusZone>;
    pub fn children_of_zone(&self, zone_key: &SpatialKey) -> impl Iterator<Item = ChildScope<'_>>;
    pub fn ancestor_zones(&self, key: &SpatialKey) -> Vec<&FocusZone>;

    pub fn push_layer(&mut self, l: FocusLayer);
    pub fn remove_layer(&mut self, key: &LayerKey);
    pub fn layer(&self, key: &LayerKey) -> Option<&FocusLayer>;
    pub fn children_of_layer(&self, key: &LayerKey) -> Vec<&FocusLayer>;
    pub fn root_for_window(&self, label: &WindowLabel) -> Option<&FocusLayer>;
    pub fn ancestors_of_layer(&self, key: &LayerKey) -> Vec<&FocusLayer>;
}

/// Variant-aware view returned by `children_of_zone`. Public, no internal-enum leak.
pub enum ChildScope<'a> {
    Leaf(&'a FocusScope),
    Zone(&'a FocusZone),
}
```

### Tauri commands — in `kanban-app/src/commands.rs`

The headless registry lives in `swissarmyhammer-focus`. Tauri adapters live in `kanban-app/src/commands.rs`, each deriving `WindowLabel` from the `tauri::Window` parameter and delegating to the focus crate. The leaf-scope command is `spatial_register_scope` (renamed from `spatial_register_focusable` in card `01KQ5PP55SAAVJ0V3HDJ1DGNBY`); the container command stays `spatial_register_zone`.

### Tests — `swissarmyhammer-focus/tests/`

Pure-Rust, no Tauri, no jsdom, no kanban. Constructs synthetic `Rect`/`SpatialKey`/`Moniker` values with `::from_string` / `Pixels(..)`.

### Design decisions

- **`swissarmyhammer-focus`, not `swissarmyhammer-kanban`**: spatial nav is generic — opaque monikers, abstract rects. Pulling it out keeps it reusable and keeps the kanban crate focused. (`swissarmyhammer-commands` stays Tier 0.)
- **`define_id!` macro reuse**: consistent with `TaskId`/`ColumnId`/`TagId`; avoids hand-rolled boilerplate.
- **Three peer types, not a `kind` field on one struct**: zone-only fields are type-checked.
- **No public sum-type enum**: registry stores leaves and zones via an internal discriminator; consumers iterate through typed accessors.
- **`Pixels` arithmetic**: prevents accidental mixing of pixel values with unrelated floats.

### Subtasks
- [x] (Skeleton + traits exist already from `01KQ2E7RPBPJ8...`)
- [x] Fill `swissarmyhammer-focus/src/types.rs`: `define_id!` newtypes; hand-roll `Pixels` + arithmetic; `Rect` + accessors; `Direction` enum
- [x] Fill `swissarmyhammer-focus/src/scope.rs`: `FocusScope` (leaf), `FocusZone`, internal `RegisteredScope` enum + helper methods
- [x] Fill `swissarmyhammer-focus/src/layer.rs`: `FocusLayer`
- [x] Fill `swissarmyhammer-focus/src/registry.rs`: `SpatialRegistry` with all scope + layer + forest + zone-tree ops; `ChildScope` variant-aware view
- [x] Tauri commands in `kanban-app/src/commands.rs` import from `swissarmyhammer_focus::*`; derive `WindowLabel` from `tauri::Window` (deferred to downstream cards that wire actual `spatial_*` handlers — this card produces the kernel they delegate to)

## Acceptance Criteria
- [x] All types live in `swissarmyhammer-focus` — NOT `swissarmyhammer-kanban`
- [x] String-valued newtypes use `define_id!`; no hand-rolled `#[serde(transparent)]` String wrappers
- [x] No bare `String` or `f64` on any public type / command signature
- [x] `FocusScope` (leaf), `FocusZone`, `FocusLayer` are distinct structs; no public sum-type enum
- [x] `Pixels` supports arithmetic without `.0` access
- [x] `swissarmyhammer-kanban/src/focus.rs` (`resolve_focused_column`) untouched
- [x] Tauri commands derive `WindowLabel` from `tauri::Window` (kernel design enforces this — concrete handlers wired by downstream cards)
- [x] `cargo test -p swissarmyhammer-focus` passes; no Tauri or jsdom required

## Tests (in `swissarmyhammer-focus/tests/focus_registry.rs`)
- [x] Each newtype JSON-round-trips as a bare primitive
- [x] `Pixels` arithmetic is type-preserving
- [x] `FocusScope` and `FocusZone` each round-trip independently (no public enum to round-trip)
- [x] Registry: register a leaf + a zone; `scope(key)` returns `Some` only for leaves, `zone(key)` only for zones
- [x] Registry: `children_of_zone` direct children only (returns `ChildScope` enum)
- [x] Registry: `ancestor_zones` walks `parent_zone` up to layer root
- [x] Registry: `children_of_layer` filtered by parent, not cross-window
- [x] Registry: `ancestors_of_layer` walks `layer.parent`
- [x] Registry: `leaves_in_layer` / `zones_in_layer` typed iterators by `layer_key`
- [x] Registry: 2 windows + 2 inspector layers + 1 dialog = 5 layers, 2 roots, correct chains

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Verification (2026-04-25)

Ran in working tree:
- `cargo build -p swissarmyhammer-focus` — clean.
- `cargo test -p swissarmyhammer-focus` — 113 tests pass across 9 binaries (lib: 22, batch_register: 12, crate_compiles: 1, drill: 11, fallback: 11, focus_registry: 18, focus_state: 7, navigate: 26, traits_object_safe: 5).
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` — no warnings.
- `cargo build --workspace` — clean.
- `swissarmyhammer-kanban/src/focus.rs` (`resolve_focused_column`) last touched in unrelated refactor commit `b81336d42`; untouched for this card.

All kernel-types acceptance criteria satisfied. Tauri command wiring belongs to the downstream cards this task `blocks`.

## Verification (2026-04-26 — three-peer collapse)

Card `01KQ5PP55SAAVJ0V3HDJ1DGNBY` rewrote the kernel as three peers:
- `Focusable` struct → `FocusScope` struct (leaf).
- Public `FocusScope` enum dropped; internal `RegisteredScope` enum stored in registry.
- Tauri command `spatial_register_focusable` → `spatial_register_scope`.
- Registry surface: `scope`/`zone` typed accessors, `leaves_in_layer`/`zones_in_layer` iterators, `ChildScope` view for variant-aware children.

`cargo test -p swissarmyhammer-focus` — 89 tests pass.