---
assignees:
- claude-code
position_column: todo
position_ordinal: e780
project: spatial-nav
title: 'stateless: implement decide() — pure-functional kernel, all motions covered'
---
## Why this is card 3

Implement the stateless kernel as a self-contained module. No consumer changes yet. Sits alongside the existing kernel.

## What to build

`swissarmyhammer-focus/src/stateless/decide.rs` — body of `decide(state, op, snapshot, window)`. Exhaustively handles every `FocusOp` variant. Returns `(FocusState, Option<FocusChangedEvent>)`.

Per-op behaviour (mirrors the algorithms validated in card 1, but reads from `snapshot` instead of `&SpatialRegistry`):

| Op | Behaviour |
|---|---|
| `Cardinal{dir}` | beam-pick nearest in half-plane, in-band bias not hard filter; `record_focus` on hit; `parent_zone` chain walked through snapshot |
| `EdgeFirst` / `EdgeLast` | `children_of(focused.parent_zone)` topmost-leftmost / bottom-rightmost |
| `DrillIn` | prefer `state.last_focused_by_fq.get(focused)`; fall back to `first_child_by_top_left` on `children_of(focused)` |
| `DrillOut` | `snapshot.get(focused).parent_zone` |
| `Click{fq}` | validate fq is in snapshot; commit + `record_focus` |
| `FocusLost{lost, lost_parent_zone, lost_layer}` | `resolve_fallback` cascade: sibling-in-zone → parent-zone last_focused → parent-zone nearest → parent-layer last_focused → parent-layer nearest → NoFocus |
| `ClearFocus` | drop `focus_by_window[window]` |
| `PushLayer` / `PopLayer` | layer stack ops; pop returns restoration target via event so the consumer commits with a snapshot |

`record_focus` walks the snapshot's `parent_zone` chain for the focused FQ, then the layer chain via `state.layers[fq].parent`, writing each ancestor's slot in `last_focused_by_fq` (scopes) or `layer.last_focused` (layers).

Cycle-guard parent_zone walks with a HashSet (logs `tracing::error!` and stops on cycle).

## Tests

Comprehensive unit suite in `swissarmyhammer-focus/src/stateless/decide.rs::tests`:

- Every cardinal direction (in-band, out-of-band reachable via score bias, no-target stay-put, override redirect, override wall, cross-layer fall-through)
- First / Last from leaf with siblings; from top-level (stay-put)
- Drill in (cold + warm); drill out (leaf + top-level)
- Focus-lost: each fallback cascade rule fires under a fixture that isolates it
- Layer push/pop: last_focused recorded, restoration target emitted

## Acceptance

- `cargo test -p swissarmyhammer-focus` green (existing tests unchanged + new decide() suite passes)
- 100% branch coverage on `decide()`
- `decide()` is `#[must_use]`, takes `&` references, never panics on torn snapshots
- No consumer-side changes
#stateless-rebuild