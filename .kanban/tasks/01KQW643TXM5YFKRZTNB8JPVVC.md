---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa680
project: spatial-nav
title: 'spatial-nav redesign step 2: add NavSnapshot / SnapshotScope types and helpers in Rust'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Define the snapshot data types in Rust and add the helpers that pathfinding / fallback / record_focus will use in steps 3–5. No callers yet; this is foundational typing work.

## What to build

### New module `swissarmyhammer-focus/src/snapshot.rs`

```rust
pub struct NavSnapshot {
    pub layer_fq: FullyQualifiedMoniker,
    pub scopes: Vec<SnapshotScope>,
}

pub struct SnapshotScope {
    pub fq: FullyQualifiedMoniker,
    pub rect: PixelRect,
    pub parent_zone: Option<FullyQualifiedMoniker>,
    pub nav_override: FocusOverrides,
}
```

Both `Serialize` / `Deserialize` (serde) so they round-trip across the Tauri IPC.

### Index helper

`NavSnapshot` in walk-heavy code paths is best treated as a flat `Vec` plus a one-time-built `HashMap<FQ, &SnapshotScope>`. Add a wrapper:

```rust
pub struct IndexedSnapshot<'a> {
    snapshot: &'a NavSnapshot,
    by_fq: HashMap<FullyQualifiedMoniker, &'a SnapshotScope>,
}

impl<'a> IndexedSnapshot<'a> {
    pub fn new(snapshot: &'a NavSnapshot) -> Self { /* build map */ }
    pub fn get(&self, fq: &FullyQualifiedMoniker) -> Option<&SnapshotScope> { ... }
    pub fn parent_zone_chain(&self, fq: &FullyQualifiedMoniker) -> impl Iterator<Item = &SnapshotScope>;
    pub fn layer_fq(&self) -> &FullyQualifiedMoniker { &self.snapshot.layer_fq }
    pub fn scopes(&self) -> &[SnapshotScope] { &self.snapshot.scopes }
}
```

`parent_zone_chain` walks from `fq` up via each entry's `parent_zone`, yielding ancestor `SnapshotScope`s. Stops at `None` or a missing FQ (torn snapshot — should not happen but degrade gracefully). Cycle-guard with a `HashSet` like `record_focus` does today.

### Module wire-up

Add `pub mod snapshot;` to `swissarmyhammer-focus/src/lib.rs`. Re-export `NavSnapshot`, `SnapshotScope`, `IndexedSnapshot` at crate root for adapter use.

## Tests

In `swissarmyhammer-focus/src/snapshot.rs::tests`:

- Round-trip serde for `NavSnapshot`
- `IndexedSnapshot::get` returns the right scope by FQ
- `parent_zone_chain` walks correctly across a 3-level chain (leaf → zone → layer-root-scope)
- `parent_zone_chain` returns empty iterator when `fq` not in snapshot
- `parent_zone_chain` breaks cleanly on a synthetic cycle (logs `tracing::error!`, does not loop)
- `parent_zone_chain` stops at first ancestor whose `parent_zone` is `None`

## Out of scope

- Adapting `geometric_pick`, `resolve_fallback`, `record_focus` (steps 3, 4, 5)
- IPC commands carrying snapshots (steps 6, 7, 8)

## Acceptance criteria

- `swissarmyhammer-focus/src/snapshot.rs` exists and compiles
- All snapshot helpers covered by unit tests
- `cargo test -p swissarmyhammer-focus` green
- No production callers yet — module is dead weight until step 3, intentionally

## Files

- `swissarmyhammer-focus/src/snapshot.rs` (new)
- `swissarmyhammer-focus/src/lib.rs` (export) #stateless-nav