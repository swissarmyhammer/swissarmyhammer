---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe480
project: spatial-nav
title: Extract spatial nav into its own crate (swissarmyhammer-spatial-nav)
---
## What

`spatial_state.rs` and `spatial_nav.rs` currently live in `swissarmyhammer-commands`, whose doc comment (`swissarmyhammer-commands/src/lib.rs:4-11`) declares the crate "consumer-agnostic — it knows nothing about kanban, tasks, or specific entity types." The spatial modules fit that philosophically, but they have **zero dependency on the Command / CommandContext / CommandsRegistry machinery** that is the actual purpose of `swissarmyhammer-commands`. They are two unrelated bodies of code sharing a crate of convenience.

### Evidence

- `spatial_state.rs` and `spatial_nav.rs` import nothing from elsewhere in `swissarmyhammer-commands` except `serde` and stdlib.
- The only consumer of `SpatialState`, `Direction`, `Rect`, `SpatialEntry`, `BatchEntry`, `FocusChanged`, `LayerEntry` is `kanban-app`.
- `swissarmyhammer-commands/Cargo.toml` has minimal deps (serde, serde_yaml_ng, async-trait, thiserror) — the spatial code fits trivially into its own crate with just serde.

### Subtasks

- [x] Create `swissarmyhammer-spatial-nav` crate (workspace member)
- [x] Move `spatial_state.rs`, `spatial_nav.rs`, and their tests into the new crate
- [x] Re-export the public surface (`Direction`, `Rect`, `SpatialEntry`, `BatchEntry`, `FocusChanged`, `LayerEntry`, `SpatialState`, `ParseDirectionError`) from its `lib.rs`
- [x] Remove spatial modules and re-exports from `swissarmyhammer-commands/src/lib.rs`
- [x] Update `kanban-app/Cargo.toml` to depend on `swissarmyhammer-spatial-nav` directly
- [x] Update imports in `kanban-app/src/spatial.rs` and `kanban-app/src/state.rs`
- [x] Verify `cargo test -p swissarmyhammer-spatial-nav` and `cargo test -p kanban-app` both pass

## Acceptance Criteria

- [x] `swissarmyhammer-commands` no longer exports anything spatial
- [x] `swissarmyhammer-spatial-nav` is a leaf crate with only serde as a meaningful dep
- [x] All existing tests pass unchanged
- [x] Workspace compiles with no warnings

## Implementation Notes

- New crate at `swissarmyhammer-spatial-nav/` with `serde` as the sole dependency.
- `spatial_nav.rs` and `spatial_state.rs` moved verbatim — internal `crate::spatial_state::` and `crate::spatial_nav::` paths remain valid in the new crate.
- `lib.rs` re-exports: `Direction`, `ParseDirectionError`, `BatchEntry`, `FocusChanged`, `LayerEntry`, `Rect`, `SpatialEntry`, `SpatialState`.
- `swissarmyhammer-commands/src/lib.rs` pruned of `spatial_nav`/`spatial_state` modules and re-exports.
- `kanban-app/src/state.rs` now imports `SpatialState` from `swissarmyhammer_spatial_nav`.
- `kanban-app/src/spatial.rs` now imports `BatchEntry`, `Direction`, `Rect`, and `ParseDirectionError` from `swissarmyhammer_spatial_nav`.
- Workspace `Cargo.toml` updated with new member + path dependency entry.

## Verification

- `cargo test -p swissarmyhammer-spatial-nav` — 50 tests pass.
- `cargo test -p swissarmyhammer-commands` — 175 tests pass.
- `cargo test -p kanban-app` — 76 tests pass.
- `cargo check --workspace` — clean.

## Pre-existing Clippy Notes

`cargo clippy -D warnings` on the moved code flags three `unnecessary_map_or` lints (suggest `is_none_or`). These lints existed verbatim in the original `swissarmyhammer-commands` location — confirmed by running clippy against `HEAD`. Not introduced by this refactor; intentionally left as-is to keep the move byte-for-byte faithful.