---
assignees:
- wballard
depends_on:
- 01KQW643TXM5YFKRZTNB8JPVVC
position_column: todo
position_ordinal: d280
project: spatial-nav
title: 'spatial-nav redesign step 3: adapt pathfinding to accept optional snapshot'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Make pathfinding (`geometric_pick`, `BeamNavStrategy::next`, override resolution) able to run against a `NavSnapshot` instead of `SpatialRegistry::scopes`. Keep the existing registry-based path working for backwards compatibility — both paths produce identical results for the same scope set.

## What to build

### Signature change

`navigate.rs::geometric_pick` (around line 345) and `BeamNavStrategy::next` (around line 224) currently consult `registry.scopes` for candidate rects, parent_zones, and overrides. Add a parallel implementation that takes `&IndexedSnapshot` instead.

Either:

(a) Add a `NavScopeView` trait that abstracts "give me the scope set, give me a scope by FQ, give me a scope's rect/parent_zone/nav_override," implement it for both `&SpatialRegistry` and `&IndexedSnapshot`, and parameterize `geometric_pick` over it. Cleanest, most code reuse.

(b) Duplicate the function bodies once for snapshot, once for registry. Simpler refactor, more code.

Prefer (a). The trait surface is small:

```rust
pub trait NavScopeView {
    fn get(&self, fq: &FullyQualifiedMoniker) -> Option<NavScopeRef<'_>>;
    fn iter(&self) -> Box<dyn Iterator<Item = NavScopeRef<'_>> + '_>;
}

pub struct NavScopeRef<'a> {
    pub fq: &'a FullyQualifiedMoniker,
    pub rect: PixelRect,
    pub parent_zone: Option<&'a FullyQualifiedMoniker>,
    pub nav_override: &'a FocusOverrides,
}
```

### Adapt `check_override`

The override-walk path (consulting nav_override for each ancestor up the parent_zone chain) currently walks `registry.scopes`. Adapt it to use `NavScopeView` so it works for both backing stores.

### Don't change call sites yet

`SpatialState::navigate` (or whatever calls `geometric_pick`) still passes `&registry`. We're not switching to snapshot-driven nav at the call boundary in this step — that comes in step 6. This step is purely internal refactoring to make snapshot paths possible.

## Tests

- Every existing `navigate.rs` test gets a parallel snapshot variant. Build a `NavSnapshot` with the same scope set as the test's registry, run pathfinding via both paths, assert identical result FQM.
- Empty half-plane (no candidates in direction): both paths return `focused_fq` (stay-put).
- nav_override redirect: both paths honor it identically.
- nav_override block (`null`): both paths return `focused_fq`.
- Cycle in `parent_zone`: handled identically by both paths (existing cycle guard reused via the trait).

## Out of scope

- Adapting `resolve_fallback` (step 4)
- Adapting `record_focus` (step 5)
- New IPC commands (steps 6-8)

## Acceptance criteria

- `cargo test -p swissarmyhammer-focus` green
- Trait + both impls compile, no warnings
- Every pathfinding test runs against both registry and snapshot paths and produces matching results
- No call site changes — `SpatialState::navigate` still uses registry path

## Files

- `swissarmyhammer-focus/src/navigate.rs` — trait, snapshot impl, registry impl, refactor `geometric_pick` / `BeamNavStrategy::next` / `check_override`
- Possibly `swissarmyhammer-focus/src/snapshot.rs` — implement `NavScopeView for IndexedSnapshot` #01KQTC1VNQM9KC90S65P7QX9N1