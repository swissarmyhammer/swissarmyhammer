---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: todo
position_ordinal: a280
project: spatial-nav
title: 'Spatial navigation algorithm in Rust: nearest-neighbor by direction'
---
## What

Implement the spatial navigation algorithm in Rust as a pure function on `SpatialRegistry`. Informed by prior art from Android FocusFinder, W3C CSS Spatial Navigation, UWP XYFocus, and Norigin Spatial Navigation.

### Algorithm — `SpatialRegistry::navigate(direction, focused_moniker) -> Option<String>`

**Two-phase candidate selection** (inspired by Android FocusFinder's beam test):

**Phase 1: Beam candidates** — A "beam" is the perpendicular band extending from the source element in the navigation direction. For nav.right, the beam is the horizontal strip from `source.top` to `source.bottom`, extending rightward. Candidates whose rects overlap this beam are "in-beam" candidates.

**Phase 2: Out-of-beam fallback** — If no in-beam candidates exist, all candidates in the correct direction are considered. This prevents dead ends in asymmetric layouts.

**Directional filter** (same as before):
- `Right`: candidates whose `rect.left >= origin.right`
- `Left`: candidates whose `rect.right <= origin.left`
- `Down`: candidates whose `rect.top >= origin.bottom`
- `Up`: candidates whose `rect.bottom <= origin.top`

**Scoring** — Android-style asymmetric squared weighting:

```rust
score = 13 * major_axis_distance² + minor_axis_distance²
```

Where:
- `major_axis_distance` = gap between source's far edge and candidate's near edge along travel direction (0 if overlapping on that axis)
- `minor_axis_distance` = distance between centers on the perpendicular axis

The 13:1 squared ratio (from Android FocusFinder, battle-tested on millions of TV devices) strongly prefers aligned candidates. A candidate 10px away on-axis but 50px off-axis scores `13*100 + 2500 = 3800`, while one 50px on-axis but 10px off-axis scores `13*2500 + 100 = 32600`. Alignment dominates.

In-beam candidates are always preferred over out-of-beam candidates regardless of score.

**Edge commands** (unchanged):
- `First`: smallest `(rect.top, rect.left)` in active layer
- `Last`: largest `(rect.bottom, rect.right)` in active layer
- `RowStart`/`RowEnd`: same-row filter (center_y ±half height), then leftmost/rightmost

**Container-first search** (inspired by W3C "local-first"):
- When navigating, first search siblings within the same parent FocusScope
- If no candidate found in the parent scope, expand to the full active layer
- This prevents focus from teleporting across the window (e.g., nav.down from a toolbar button shouldn't skip the column headers to land on a card)
- Requires the registry to store parent scope relationships (already available from CommandScope's parent chain)

### Focus memory per layer (inspired by LRUD-spatial)

When a FocusLayer becomes inactive (another layer pushes on top), store the last-focused moniker. When the layer becomes active again (upper layer pops), restore focus to the remembered moniker rather than defaulting to First. This is stored in the `LayerStack` entry.

### Files to create/modify

**Rust** — `swissarmyhammer-kanban/src/spatial.rs` (or `swissarmyhammer-commands/src/spatial.rs`):
- Add `Direction` enum: `Up, Down, Left, Right, First, Last, RowStart, RowEnd`
- Add `SpatialRegistry::navigate(&self, direction, focused) -> Option<String>` with beam test + scoring
- Store `parent_scope: Option<String>` in `SpatialEntry` for container-first search
- Store `last_focused: Option<String>` in `LayerEntry` for focus memory

**React** — `kanban-app/ui/src/lib/entity-focus-context.tsx`:
- `broadcastNavCommand` stays synchronous. Spatial nav is fire-and-forget via `.then()`.
- `claimWhen` predicates evaluated first (backward compat), spatial fallback second.

### Design decisions
- **Beam test first**: Matches the strong human intuition that "right" means "directly to the right". Android has validated this over 10+ years.
- **13:1 squared ratio**: Not arbitrary — it's Android's production constant. Configurable later if needed, but start with the proven value.
- **Container-first search**: Prevents teleportation. If a toolbar button's nav.down should hit the toolbar's next row (not a board card), the parent scope constraint handles it.
- **Focus memory per layer**: When inspector closes and window layer resumes, focus returns to the exact card you were on, not the first card.
- **broadcastNavCommand stays synchronous**: Fire-and-forget via `.then()`. No breaking API change.
- **Layer boundaries are hard**: Navigation cannot escape the active layer.

### Subtasks
- [ ] Implement beam test: filter candidates into in-beam and out-of-beam sets
- [ ] Implement Android-style scoring: `13 * major² + minor²`
- [ ] Implement container-first search: parent scope siblings first, then full layer
- [ ] Implement edge commands (First, Last, RowStart, RowEnd)
- [ ] Add focus memory to LayerStack entries

## Acceptance Criteria
- [ ] In-beam candidates always preferred over out-of-beam
- [ ] Aligned candidates preferred over closer-but-diagonal ones (13:1 ratio)
- [ ] Container-first: nav.down from toolbar stays in toolbar row if siblings exist
- [ ] Container fallback: nav.down from last toolbar row falls through to board
- [ ] Focus memory: layer pop restores last-focused moniker from that layer
- [ ] All 8 directions produce correct results
- [ ] Navigation stays within active layer (hard boundary)
- [ ] `broadcastNavCommand` stays synchronous
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests
- [ ] `Rust unit tests` — Beam test: in-beam candidate preferred over closer out-of-beam candidate
- [ ] `Rust unit tests` — Scoring: aligned candidate at 50px beats diagonal candidate at 20px
- [ ] `Rust unit tests` — Container-first: siblings in parent scope found before distant elements
- [ ] `Rust unit tests` — Container fallback: no sibling → falls through to full layer search
- [ ] `Rust unit tests` — Focus memory: layer push/pop stores and restores last-focused
- [ ] `Rust unit tests` — Realistic board: 3 columns × 5 cards, all directions correct
- [ ] `Rust unit tests` — Cross-column clamping: tall column → short column lands on nearest
- [ ] `Rust unit tests` — Empty column: nav.right lands on column header
- [ ] `Rust unit tests` — Inspector: 8 stacked fields, up/down/first/last correct
- [ ] `entity-focus-context.test.tsx` — broadcastNavCommand stays synchronous, returns boolean
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.