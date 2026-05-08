---
assignees:
- claude-code
position_column: todo
position_ordinal: e780
project: spatial-nav
title: 'stateless: implement decide() — pure-functional kernel, all motions covered'
---
## Why this is card 3

Implement the stateless kernel as a self-contained module. Sits alongside the existing kernel during this card; card 5 deletes the old kernel.

**End-state goal:** `swissarmyhammer-focus` is a *stateless* crate. The only computation surface is `decide(state, op, snapshot, window) -> FocusDecision`. The crate holds no global state, no `Mutex`, no `OnceCell`, no `static mut`, no interior mutability — every byte of mutable state lives in the **consumer**, which threads `&FocusState` and `&NavSnapshot` in on every call. This card writes `decide()` to that contract; cards 4 and 5 enforce it across the rest of the surface.

**Folds in card 1's work.** The motions-fix card (`01KQZ7VR7JK1QD5QJDDKB529JG`) was archived as redundant. Its three sub-fixes are absorbed here:
- (1) **in-band score-bias** — the existing `BeamNavStrategy::geometric_pick` drops candidates that don't strictly overlap the focused scope's perpendicular band (the hard `if !in_beam { continue; }` filter at `navigate.rs:355-357`, `:584-587`, `:611-613`). `decide()`'s `Cardinal` arm replaces that hard filter with a *score bias*: in-band candidates score lower (better), out-of-band candidates remain reachable when no in-band target exists. This matches the Android beam-search shape.
- (2) **first/last on leaves** — already shipped in HEAD post-`d0460d061` (`navigate.rs:471-493`). `decide()`'s `EdgeFirst` / `EdgeLast` arms preserve the same semantics: walk `children_of(focused.parent_zone)` for siblings; fall back to `children_of(focused)` at the layer root.
- (3) **drill in/out verification** — verify `decide()`'s `DrillIn` / `DrillOut` arms produce the same results as `SpatialRegistry::drill_in` / `drill_out`. Add the parametrized regression tests card 1 specified.

## What to build

`swissarmyhammer-focus/src/stateless/decide.rs` — body of `decide(state, op, snapshot, window)`. Exhaustively handles every `FocusOp` variant. Returns `FocusDecision { next: FocusState, event: Option<FocusChangedEvent> }`.

```rust
#[must_use]
pub fn decide(
    state: &FocusState,
    op: &FocusOp,
    snapshot: &NavSnapshot,
    window: &WindowLabel,
) -> FocusDecision { ... }
```

Per-op behaviour (mirrors the algorithms validated against the existing kernel, but reads from `snapshot` instead of `&SpatialRegistry`):

| Op | Behaviour |
|---|---|
| `Cardinal{dir}` | beam-pick nearest in half-plane; **in-band is a score bias, not a hard filter** (out-of-band candidates remain reachable when no in-band target exists — see card 1 fold-in note above); `record_focus` on hit; `parent_zone` chain walked through snapshot |
| `EdgeFirst` / `EdgeLast` | `children_of(focused.parent_zone)` topmost-leftmost / bottom-rightmost; layer root falls back to `children_of(focused)` |
| `DrillIn` | prefer `state.last_focused_by_fq.get(focused)`; fall back to `first_child_by_top_left` on `children_of(focused)` |
| `DrillOut` | `snapshot.get(focused).parent_zone` |
| `Click{fq}` | validate fq is in snapshot; commit + `record_focus` |
| `FocusLost{lost, lost_parent_zone, lost_layer}` | `resolve_fallback` cascade: sibling-in-zone → parent-zone last_focused → parent-zone nearest → parent-layer last_focused → parent-layer nearest → NoFocus |
| `ClearFocus` | drop `focus_by_window[window]` |
| `PushLayer` / `PopLayer` | layer stack ops; pop returns restoration target via event so the consumer commits with a snapshot |

`record_focus` walks the snapshot's `parent_zone` chain for the focused FQ, then the layer chain via `state.layers[fq].parent`, writing each ancestor's slot in `last_focused_by_fq` (scopes) or `layer.last_focused` (layers).

Cycle-guard parent_zone walks with a HashSet (logs `tracing::error!` and stops on cycle).

### Statelessness invariants this card enforces

- `decide()` is `#[must_use]`, takes `&` references for all four parameters, and returns the new `FocusState` by value inside `FocusDecision`.
- No file under `swissarmyhammer-focus/src/stateless/` uses `Mutex`, `RwLock`, `OnceCell`, `Lazy`, `lazy_static!`, `static mut`, `RefCell`, `Cell`, or `parking_lot::*`. The module is pure.
- All helpers under `stateless/` take `&NavSnapshot`, never `&SpatialRegistry`. The snapshot is the only source of structural data.
- No `tokio` / `async` anywhere in `stateless/` — `decide()` is sync and CPU-bound.

## Tests

Comprehensive unit suite in `swissarmyhammer-focus/src/stateless/decide.rs::tests`:

- **Cardinal directions** — in-band hit, out-of-band reachable via score bias (the card-1 regression case: nav-rail thin row → board area), no-target stay-put, override redirect, override wall, cross-layer fall-through. Specifically pin: ArrowRight from a thin nav-rail leaf lands on the nearest board-area scope, NOT stays-put. This is the bug the in-band hard filter caused on the legacy kernel.
- **First / Last** — from leaf with siblings (parametrized over leaf rect positions), from top-level (children-of-self fallback), parent_zone == None (stay-put).
- **Drill in** — cold-start (no `last_focused`) → `first_child_by_top_left`; warm-start (`last_focused` set) → that child. Compare-tested against `SpatialRegistry::drill_in` until card 5 deletes the legacy path.
- **Drill out** — leaf → `parent_zone`; top-level scope (no parent) → focused (stay-put). Compare-tested against `SpatialRegistry::drill_out`.
- **Focus-lost** — each fallback cascade rule fires under a fixture that isolates it.
- **Layer push/pop** — last_focused recorded, restoration target emitted.

Plus structural tests:

- `swissarmyhammer-focus/tests/stateless_is_pure.rs`: a regex-grep over `src/stateless/**/*.rs` (using `include_str!` of each file) that asserts none of `Mutex`, `RwLock`, `OnceCell`, `Lazy`, `lazy_static`, `static mut`, `RefCell`, `Cell`, `parking_lot::`, `tokio::sync::` appear in any file. Test fails ⇔ someone reintroduces interior mutability.
- `swissarmyhammer-focus/tests/decide_signature.rs`: a one-line `let _: fn(&FocusState, &FocusOp, &NavSnapshot, &WindowLabel) -> FocusDecision = swissarmyhammer_focus::stateless::decide;` — pins the by-reference signature so any drift to `&mut` or owned values fails to compile.

## Acceptance

- `cargo test -p swissarmyhammer-focus` green (existing tests unchanged + new decide() suite passes + stateless_is_pure + decide_signature pass).
- 100% branch coverage on `decide()`.
- `decide()` is `#[must_use]`, takes `&` references, never panics on torn snapshots.
- `src/stateless/` directory contains zero interior-mutability primitives (asserted by `stateless_is_pure.rs`).
- The card-1 regression scenario (ArrowRight from nav-rail leaf reaches board area) passes against `decide()`.
- No consumer-side changes (card 4 owns those).

#stateless-rebuild #stateless-nav