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

### Algorithm — `SpatialRegistry::navigate(key, direction) -> Result<()>`

Looks up the key's rect, runs beam test + scoring against candidates in the active layer, updates `focused_key`, emits `focus-changed` event.

**Two-phase candidate selection** (Android FocusFinder beam test):

**Phase 1: Beam candidates** — A "beam" is the perpendicular band extending from the source in the nav direction. For Right, the beam spans `source.top..source.bottom` extending rightward. Candidates overlapping this band are "in-beam."

**Phase 2: Out-of-beam fallback** — If no in-beam candidates, all candidates in the correct direction are considered.

**Directional filter**:
- `Right`: `candidate.left >= origin.right`
- `Left`: `candidate.right <= origin.left`
- `Down`: `candidate.top >= origin.bottom`
- `Up`: `candidate.bottom <= origin.top`

**Scoring** — Android-style: `13 * major² + minor²`

**Edge commands**: First (top-left-most), Last (bottom-right-most), RowStart/RowEnd (same-row filter).

**Container-first search**: Search siblings in parent scope first, expand to full layer if empty.

**Focus memory per layer**: LayerStack stores last-focused key per layer, restores on layer pop.

### Files to create/modify

**Rust** — `swissarmyhammer-commands/src/spatial.rs`:
- `Direction` enum (8 variants)
- `SpatialRegistry::navigate(&mut self, key, direction) -> Result<()>` with beam test + scoring
- `parent_scope: Option<String>` in `SpatialEntry` for container-first
- `last_focused: Option<String>` in `LayerEntry` for focus memory

**React** — `kanban-app/ui/src/lib/entity-focus-context.tsx`:
- `broadcastNavCommand` stays synchronous. Calls `invoke("spatial_navigate", { key, direction })` fire-and-forget.

### Design decisions
- **Beam test first**: Android-validated, 10+ years.
- **13:1 squared ratio**: Android's production constant.
- **Container-first**: Prevents teleportation across the window.
- **Focus memory per layer**: Inspector close returns to exact previous card.
- **Layer boundaries are hard**: Nav cannot escape the active layer.

### Subtasks
- [ ] Implement beam test: filter candidates into in-beam and out-of-beam sets
- [ ] Implement Android-style scoring: `13 * major² + minor²`
- [ ] Implement container-first search: parent scope siblings first, then full layer
- [ ] Implement edge commands (First, Last, RowStart, RowEnd)
- [ ] Add focus memory to LayerStack entries

## Acceptance Criteria
- [ ] In-beam candidates always preferred over out-of-beam
- [ ] Aligned candidates preferred over closer-but-diagonal (13:1 ratio)
- [ ] Container-first: nav stays in parent scope if siblings exist
- [ ] Container fallback: no sibling → expands to full layer
- [ ] Focus memory: layer pop restores last-focused key
- [ ] All 8 directions correct
- [ ] Hard layer boundary — entries in other layers excluded
- [ ] `cargo test` passes

## Tests

All tests are pure Rust with synthetic rects — no DOM, no React.

### Beam test

```rust
#[test]
fn beam_candidate_preferred_over_closer_out_of_beam() {
    // source at (0, 100, 100, 50) — a 100x50 rect, top-left at (0,100)
    // candidate A at (150, 120, 50, 30) — in beam (y overlap with source), 50px right
    // candidate B at (110, 10, 50, 30) — out of beam (y=10, above source), only 10px right
    // nav.right → expect A (in-beam wins over closer out-of-beam)
}
```

### Scoring

```rust
#[test]
fn aligned_candidate_beats_closer_diagonal() {
    // source at (0, 0, 100, 50)
    // candidate A at (200, 0, 100, 50) — aligned, 100px right
    //   score = 13 * 100² + 0² = 130_000
    // candidate B at (110, 200, 100, 50) — diagonal, 10px right but 175px down
    //   score = 13 * 10² + 175² = 1_300 + 30_625 = 31_925
    // Both in-beam? B is out of beam (y=200 > source.bottom=50)
    // So only A is in beam → A wins by beam, not even by score
}

#[test]
fn scoring_tiebreak_among_in_beam_candidates() {
    // source at (0, 0, 100, 100)
    // candidate A at (200, 0, 100, 100) — aligned, 100px right
    //   score = 13 * 100² + 0² = 130_000
    // candidate B at (150, 20, 100, 100) — aligned, 50px right, 20px down
    //   score = 13 * 50² + 20² = 32_500 + 400 = 32_900
    // Both in-beam → B wins (lower score)
}
```

### Realistic board layout — 3 columns × varying cards

```rust
#[test]
fn board_layout_3_columns() {
    // Column 0 header at (0, 0, 200, 40), cards at (0, 50, 200, 60), (0, 120, 200, 60), ...
    // Column 1 header at (210, 0, 200, 40), cards at (210, 50, 200, 60), (210, 120, 200, 60), ...
    // Column 2 header at (420, 0, 200, 40), 3 cards only
    //
    // Verify:
    // - nav.down from col0.card[0] → col0.card[1]
    // - nav.right from col0.card[0] → col1.card[0] (beam-aligned)
    // - nav.right from col0.card[4] → col2.card[2] (clamped: col2 only has 3 cards, nearest is last)
    // - nav.up from col0.card[0] → col0.header
    // - nav.right from col2.card[2] → None (no column to the right)
    // - nav.first → col0.header (top-left-most)
    // - nav.last → col2.card[2] (bottom-right-most)
}

#[test]
fn empty_column_nav_right_lands_on_header() {
    // Column 0 with 5 cards, Column 1 with header only (no cards)
    // nav.right from col0.card[2] → col1.header (only element in col1's x-range)
}
```

### Inspector layout — stacked field rows

```rust
#[test]
fn inspector_8_stacked_fields() {
    // 8 field rows at (0, 0, 300, 35), (0, 40, 300, 35), ..., (0, 280, 300, 35)
    // nav.down from field[0] → field[1]
    // nav.up from field[7] → field[6]
    // nav.first → field[0]
    // nav.last → field[7]
    // nav.left from field[3] → None (nothing to the left)
}
```

### Pill navigation within a field row

```rust
#[test]
fn pill_horizontal_navigation() {
    // Field label at (0, 0, 100, 30)
    // Pill A at (110, 5, 60, 20), Pill B at (180, 5, 60, 20), Pill C at (250, 5, 60, 20)
    // nav.right from label → Pill A (nearest right, in beam)
    // nav.right from Pill A → Pill B
    // nav.left from Pill A → label
    // nav.left from Pill C → Pill B
}
```

### Layer isolation

```rust
#[test]
fn navigate_only_sees_active_layer() {
    // window layer: card at (0, 0, 200, 60)
    // inspector layer (active): field at (300, 0, 200, 35), field at (300, 40, 200, 35)
    // nav.right from inspector.field[0] → None (card is in window layer, invisible)
    // nav.down from inspector.field[0] → inspector.field[1]
}
```

### Container-first search

```rust
#[test]
fn container_first_stays_in_parent_scope() {
    // Toolbar row: button A at (0, 0, 80, 30), button B at (90, 0, 80, 30)
    //   parent_scope = "toolbar"
    // Board card at (0, 40, 200, 60)
    //   parent_scope = "column:todo"
    // nav.right from button A → button B (same parent scope)
    // NOT board card (which is closer vertically but different scope)
}

#[test]
fn container_fallback_when_no_sibling() {
    // Toolbar row: single button at (0, 0, 80, 30), parent_scope = "toolbar"
    // Board card at (0, 40, 200, 60), parent_scope = "column:todo"
    // nav.down from button → board card (no sibling in toolbar below, falls through to layer)
}
```

### Focus memory

```rust
#[test]
fn layer_pop_restores_last_focused() {
    // window layer: card-A focused
    // push inspector layer, focus field-1, then field-3
    // pop inspector layer
    // → focused_key restored to card-A (window layer's last_focused)
}
```

### Edge cases

```rust
#[test]
fn navigate_with_unknown_key_returns_error() {
    // spatial_navigate("nonexistent", Right) → Err
}

#[test]
fn navigate_with_no_candidates_is_noop() {
    // Single entry in layer, nav.right → no event emitted, focused_key unchanged
}

#[test]
fn navigate_rowstart_rowend() {
    // 3x3 grid: row 0 at y=0, row 1 at y=50, row 2 at y=100
    // Focused on (1,1) — center of grid
    // nav.rowStart → (0,1) — leftmost in same row
    // nav.rowEnd → (2,1) — rightmost in same row
}
```

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.