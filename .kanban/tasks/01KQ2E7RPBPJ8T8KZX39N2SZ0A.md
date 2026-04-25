---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9780
project: spatial-nav
title: 'Refactor: extract spatial-nav out of swissarmyhammer-kanban into new swissarmyhammer-focus crate, add extension traits'
---
## What

The kernel work has already been built at `swissarmyhammer-kanban/src/focus/` (per the in-flight kernel card). That was the wrong crate — spatial nav has zero kanban-domain dependencies (opaque `Moniker` strings, abstract `Rect`s, `WindowLabel`s). This card pulls the existing implementation **out** of `swissarmyhammer-kanban` into a new dedicated crate `swissarmyhammer-focus`, and adds two pluggable extension traits (`NavStrategy`, `FocusEventSink`).

This is a **refactor**, not a greenfield build. The Rust code is already written; we're moving it.

### Current state to migrate

```
swissarmyhammer-kanban/src/focus/
  mod.rs            ← move out  (re-exports become crate root in new crate)
  types.rs          ← move out  (newtypes, Rect, Direction, Pixels)
  scope.rs          ← move out  (Focusable, FocusZone, FocusScope enum)
  layer.rs          ← move out  (FocusLayer)
  registry.rs       ← move out  (SpatialRegistry)
  state.rs          ← move out  (SpatialState)
  column.rs         ← STAYS — kanban-specific resolve_focused_column
```

### Target

```
swissarmyhammer-focus/
  Cargo.toml        ← new
  src/
    lib.rs          ← module decls + crate-level docs
    types.rs        ← from kanban/focus/types.rs
    scope.rs        ← from kanban/focus/scope.rs
    layer.rs        ← from kanban/focus/layer.rs
    registry.rs     ← from kanban/focus/registry.rs
    state.rs        ← from kanban/focus/state.rs
    navigate.rs     ← NEW: NavStrategy trait + BeamNavStrategy stub for card 01KNQXXF5W
    observer.rs     ← NEW: FocusEventSink trait + NoopSink + RecordingSink
  tests/            ← migrate any focus-related tests from kanban/tests/

swissarmyhammer-kanban/src/
  focus.rs          ← restored: only resolve_focused_column (was focus/column.rs)
  (focus/ directory removed)
```

The `swissarmyhammer-kanban/src/focus/column.rs` content moves back to `swissarmyhammer-kanban/src/focus.rs` (its original location before the kernel work), since it's the only kanban-specific focus code and the `focus/` directory is no longer warranted.

### Cargo.toml for the new crate

```toml
[package]
name = "swissarmyhammer-focus"
version = "0.1.0"
edition = "2024"
description = "Spatial focus and keyboard navigation engine — generic, no domain dependencies"
license = "MIT OR Apache-2.0"

[dependencies]
serde = { workspace = true, features = ["derive"] }
swissarmyhammer-common = { path = "../swissarmyhammer-common" }   # for define_id!
ulid = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

Add to the workspace root `Cargo.toml` `members` list. **No** Tauri / kanban / commands deps — the crate must compile in isolation with only `serde`, `ulid`, and `swissarmyhammer-common`.

`swissarmyhammer-kanban/Cargo.toml` adds `swissarmyhammer-focus = { path = "../swissarmyhammer-focus" }` if it still needs to reference any of these types (it shouldn't — column.rs doesn't use them).

### Public extension traits — added during the move

#### `NavStrategy` — pluggable navigation algorithm

```rust
pub trait NavStrategy: Send + Sync {
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused: &SpatialKey,
        direction: Direction,
    ) -> Option<Moniker>;
}

/// Default Android-beam-search strategy. Implementation is filled by card 01KNQXXF5W
/// (which was already targeting this signature, just under a different crate).
pub struct BeamNavStrategy;
impl NavStrategy for BeamNavStrategy { /* ... */ }
```

If the in-flight beam-search code is already written in `kanban/focus/`, move it into `BeamNavStrategy::next` as part of this refactor.

#### `FocusEventSink` — pluggable event emission

```rust
pub trait FocusEventSink: Send + Sync {
    fn emit(&self, event: &FocusChangedEvent);
}

pub struct NoopSink;
impl FocusEventSink for NoopSink {
    fn emit(&self, _: &FocusChangedEvent) {}
}

pub struct RecordingSink {
    pub events: std::sync::Mutex<Vec<FocusChangedEvent>>,
}
impl FocusEventSink for RecordingSink {
    fn emit(&self, event: &FocusChangedEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}
```

`SpatialState` methods continue returning `Option<FocusChangedEvent>` (consumers using return-value style remain unchanged); the sink is optional sugar.

### Why no trait for the registry itself

`SpatialRegistry` is a concrete value type — consumers own one and mutate via methods. No plausible alternate impl, no need for trait abstraction over hot paths.

### Import-site updates

After the move:
- `swissarmyhammer-kanban` no longer references the spatial-nav types. Audit and remove `pub use focus::{...}` re-exports of the moved items.
- `kanban-app/src/commands.rs` (Tauri adapters) imports from `swissarmyhammer_focus::*` instead of `swissarmyhammer_kanban::focus::*`. Same with any other consumer.
- `swissarmyhammer-kanban` keeps only `pub use focus::resolve_focused_column;` (or whatever the current public path was).

### Subtasks
- [ ] Create `swissarmyhammer-focus/` directory + `Cargo.toml`; add to workspace `members`
- [ ] Move `swissarmyhammer-kanban/src/focus/{types,scope,layer,registry,state}.rs` → `swissarmyhammer-focus/src/`
- [ ] Move beam-search code (currently in kanban/focus/, possibly in registry.rs or state.rs) → `swissarmyhammer-focus/src/navigate.rs` as `BeamNavStrategy::next` impl + `NavStrategy` trait
- [ ] Add `swissarmyhammer-focus/src/observer.rs` with `FocusEventSink` + `NoopSink` + `RecordingSink`
- [ ] Author `swissarmyhammer-focus/src/lib.rs` with module decls + crate-level docs
- [ ] Restore `swissarmyhammer-kanban/src/focus.rs` from `focus/column.rs` content; remove the now-empty `focus/` directory
- [ ] Update `swissarmyhammer-kanban/src/lib.rs` re-exports to drop spatial-nav types and keep only `resolve_focused_column`
- [ ] Update `kanban-app/Cargo.toml` to add `swissarmyhammer-focus` dep
- [ ] Update `kanban-app/src/commands.rs` imports from `swissarmyhammer_kanban::focus::*` → `swissarmyhammer_focus::*`
- [ ] Move focus-related tests from `swissarmyhammer-kanban/tests/` → `swissarmyhammer-focus/tests/`
- [ ] Run `cargo build` at workspace root — workspace still builds
- [ ] Run `cargo test -p swissarmyhammer-focus` — all moved tests pass

## Acceptance Criteria
- [ ] `swissarmyhammer-focus` exists with the modules above
- [ ] No kanban / Tauri / commands deps in `swissarmyhammer-focus/Cargo.toml`
- [ ] Crate compiles in isolation: `cargo build -p swissarmyhammer-focus`
- [ ] `swissarmyhammer-kanban/src/focus.rs` exists as a flat file containing only `resolve_focused_column` (and friends from the original column.rs)
- [ ] No `swissarmyhammer-kanban/src/focus/` directory
- [ ] `NavStrategy` trait + `BeamNavStrategy` impl present and used by `SpatialState::navigate` (or whatever invokes the strategy)
- [ ] `FocusEventSink` trait + `NoopSink` + `RecordingSink` present and object-safe
- [ ] All existing focus tests still pass after the move (`cargo test -p swissarmyhammer-focus`)
- [ ] Workspace `cargo build` succeeds; no new clippy warnings
- [ ] `kanban-app/src/commands.rs` imports from `swissarmyhammer_focus::*`

## Tests
- [ ] `swissarmyhammer-focus/tests/crate_compiles.rs` — minimal smoke test importing each public type
- [ ] `swissarmyhammer-focus/tests/traits_object_safe.rs` — `let _: Box<dyn NavStrategy> = ...; let _: Box<dyn FocusEventSink> = ...;`
- [ ] `RecordingSink` test: emit two events, assert both collected in order
- [ ] All previously-passing focus tests still pass after migration (run `cargo test -p swissarmyhammer-focus` and confirm count matches pre-move)
- [ ] `cargo build` and `cargo test` at workspace root pass

## Workflow
- This is a refactor with mechanical file moves. Move first, fix imports, then layer in the two new traits. Each step should keep `cargo build` green.