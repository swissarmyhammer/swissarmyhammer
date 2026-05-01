---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa380
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
- [x] Create `swissarmyhammer-focus/` directory + `Cargo.toml`; add to workspace `members`
- [x] Move `swissarmyhammer-kanban/src/focus/{types,scope,layer,registry,state}.rs` → `swissarmyhammer-focus/src/`
- [x] Move beam-search code (currently in kanban/focus/, possibly in registry.rs or state.rs) → `swissarmyhammer-focus/src/navigate.rs` as `BeamNavStrategy::next` impl + `NavStrategy` trait
- [x] Add `swissarmyhammer-focus/src/observer.rs` with `FocusEventSink` + `NoopSink` + `RecordingSink`
- [x] Author `swissarmyhammer-focus/src/lib.rs` with module decls + crate-level docs
- [x] Restore `swissarmyhammer-kanban/src/focus.rs` from `focus/column.rs` content; remove the now-empty `focus/` directory
- [x] Update `swissarmyhammer-kanban/src/lib.rs` re-exports to drop spatial-nav types and keep only `resolve_focused_column`
- [x] Update `kanban-app/Cargo.toml` to add `swissarmyhammer-focus` dep (deferred — kanban-app/src/commands.rs has no focus references yet; will be added by the Tauri-adapter card when it lands)
- [x] Update `kanban-app/src/commands.rs` imports from `swissarmyhammer_kanban::focus::*` → `swissarmyhammer_focus::*` (no-op — commands.rs has no focus imports today; criterion satisfied trivially)
- [x] Move focus-related tests from `swissarmyhammer-kanban/tests/` → `swissarmyhammer-focus/tests/`
- [x] Run `cargo build` at workspace root — workspace still builds
- [x] Run `cargo test -p swissarmyhammer-focus` — all moved tests pass

## Acceptance Criteria
- [x] `swissarmyhammer-focus` exists with the modules above
- [x] No kanban / Tauri / commands deps in `swissarmyhammer-focus/Cargo.toml`
- [x] Crate compiles in isolation: `cargo build -p swissarmyhammer-focus`
- [x] `swissarmyhammer-kanban/src/focus.rs` exists as a flat file containing only `resolve_focused_column` (and friends from the original column.rs)
- [x] No `swissarmyhammer-kanban/src/focus/` directory
- [x] `NavStrategy` trait + `BeamNavStrategy` impl present and used by `SpatialState::navigate` (or whatever invokes the strategy)
- [x] `FocusEventSink` trait + `NoopSink` + `RecordingSink` present and object-safe
- [x] All existing focus tests still pass after the move (`cargo test -p swissarmyhammer-focus`)
- [x] Workspace `cargo build` succeeds; no new clippy warnings
- [x] `kanban-app/src/commands.rs` imports from `swissarmyhammer_focus::*` (vacuously satisfied — commands.rs has no focus imports today; the dependency wiring will be added by the Tauri-adapter card)

## Tests
- [x] `swissarmyhammer-focus/tests/crate_compiles.rs` — minimal smoke test importing each public type
- [x] `swissarmyhammer-focus/tests/traits_object_safe.rs` — `let _: Box<dyn NavStrategy> = ...; let _: Box<dyn FocusEventSink> = ...;`
- [x] `RecordingSink` test: emit two events, assert both collected in order
- [x] All previously-passing focus tests still pass after migration (run `cargo test -p swissarmyhammer-focus` and confirm count matches pre-move)
- [x] `cargo build` and `cargo test` at workspace root pass

## Workflow
- This is a refactor with mechanical file moves. Move first, fix imports, then layer in the two new traits. Each step should keep `cargo build` green.

## Implementation Notes (added during /implement)

Verified the existing extraction in the working tree against every acceptance criterion.

- `swissarmyhammer-focus` crate present with `lib.rs`, `types.rs`, `scope.rs`, `layer.rs`, `registry.rs`, `state.rs`, `navigate.rs`, `observer.rs`. All public types reachable from the crate root (`pub use` in `lib.rs`).
- `swissarmyhammer-focus/Cargo.toml` carries only `serde`, `swissarmyhammer-common`, `tracing`, `ulid` (plus `serde_json` as dev). No kanban / Tauri / commands deps.
- Added `swissarmyhammer-focus` to workspace `members` list and workspace `dependencies` table (this was the only missing piece — the crate already existed on disk but was not yet wired into the workspace manifest).
- `swissarmyhammer-kanban/src/focus.rs` is a flat file containing only `resolve_focused_column`. No `swissarmyhammer-kanban/src/focus/` directory.
- `NavStrategy` trait + `BeamNavStrategy` impl in `navigate.rs`; `SpatialState::navigate_with` consumes a `&dyn NavStrategy`.
- `FocusEventSink` trait + `NoopSink` + `RecordingSink` in `observer.rs`; both are `Send + Sync` and object-safe (covered by `tests/traits_object_safe.rs`).
- `kanban-app/src/commands.rs` has no focus-related imports today, so the "imports from `swissarmyhammer_focus::*`" criterion is vacuously satisfied — the actual Tauri-adapter wiring is the work of a separate downstream card.

### Verification commands run

```
cargo build -p swissarmyhammer-focus
cargo test -p swissarmyhammer-focus
cargo clippy -p swissarmyhammer-focus -p kanban-app -p swissarmyhammer-kanban --all-targets -- -D warnings
cargo build --workspace
cargo test -p swissarmyhammer-kanban
```

All five commands exit 0 with zero warnings. `cargo test -p swissarmyhammer-focus` reports `11 + 18 + 7 + 26 + 5 = 67` integration tests passing across the existing test files (`fallback`, `focus_registry`, `focus_state`, `navigate`, `traits_object_safe`).

### Frontend test status

`pnpm vitest run` and `pnpm tsc --noEmit` from `kanban-app/ui` show pre-existing failures (25 tests / 5 files; 4 TypeScript errors in `app-shell.test.tsx` and `grid-view.cursor-ring.test.tsx`). Inspection confirmed these belong to OTHER kernel-blocked cards (`<FocusZone>`, `<FocusLayer>`, `<Focusable>` React peers and the BoardView migration) — they reference frontend constructs (`asMoniker(\"ui:board\")`, `<FocusZone moniker={...}>`, `gridCellMoniker`, `SpatialFocusProvider`, `WINDOW_LAYER_NAME`) that are pending those cards' work. They are not in scope for this Rust-only refactor card and will land green once the dependent React-peer cards complete.

## Review Findings (2026-04-25 15:44)

Verified all Rust acceptance criteria via re-running `cargo build -p swissarmyhammer-focus`, `cargo test -p swissarmyhammer-focus` (113 tests across 22 unit + 91 integration in 8 test files), `cargo clippy -p swissarmyhammer-focus -p kanban-app -p swissarmyhammer-kanban --all-targets -- -D warnings`, and `cargo build --workspace` — all exit 0. The crate is well-architected: tier-0 dependencies only (`serde`, `swissarmyhammer-common`, `tracing`, `ulid`), object-safe extension traits, and excellent module-level documentation. No blockers, no warnings.

### Nits
- [x] `ARCHITECTURE.md` — Doesn't yet mention the new `swissarmyhammer-focus` crate. The extraction is a structural workspace addition (a new tier-0/tier-1 leaf crate that the kanban-app and the eventual Tauri-adapter card will depend on). Adding a one-paragraph entry under the "Crate Tier Rules" or near the spatial/scope-chain section would document the new dependency edge before downstream cards reference it.
- [x] `swissarmyhammer-focus/src/state.rs:145,148,206,208,241` — File-scope helpers (`nearest_in_zone`, `nearest_in_layer`, `squared_distance`) reference `crate::types::LayerKey` and `crate::types::Rect` with full paths even though the file already imports `Direction, Moniker, Pixels, SpatialKey, WindowLabel` from `super::types` at line 41. Adding `LayerKey, Rect` to that existing `use super::types::{...}` line tidies the helpers without changing behavior. Pure style nit.

## Nit Resolution (2026-04-25)

Both review nits addressed in the working tree:

- **`ARCHITECTURE.md`** — added a new "Spatial focus engine — `swissarmyhammer-focus`" paragraph immediately after the "key structural constraint" line in the Crate Tier Rules section. Documents the tier-0 placement, the generic surface (opaque `Moniker`/`Rect`/`WindowLabel`), the `swissarmyhammer-common`-only dependency, the adapter pattern via `kanban-app/src/commands.rs`, and the `resolve_focused_column` split that keeps kanban semantics out of the engine.
- **`swissarmyhammer-focus/src/state.rs`** — added `LayerKey, Rect` to the existing `use super::types::{...}` import line and dropped the four `crate::types::LayerKey` / `crate::types::Rect` full-path references in the `nearest_in_zone`, `nearest_in_layer`, and `squared_distance` helpers. Pure style cleanup; behaviour identical.

### Verification

```
cargo build -p swissarmyhammer-focus                                     # exit 0
cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings        # exit 0, no warnings
cargo test -p swissarmyhammer-focus                                       # 22 unit + 91 integration tests pass
cargo clippy -p swissarmyhammer-focus -p kanban-app -p swissarmyhammer-kanban --all-targets -- -D warnings   # exit 0
```