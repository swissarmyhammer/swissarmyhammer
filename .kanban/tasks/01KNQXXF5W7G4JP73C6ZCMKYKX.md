---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa580
project: spatial-nav
title: 'Spatial nav algorithm: three-rule beam search (newtype-only signatures)'
---
## What

Implement the spatial navigation algorithm in Rust as a pure function on `SpatialRegistry`. Returns `Option<Moniker>` — newtyped, never bare `Option<String>`. Informed by Android FocusFinder (beam test + scoring), W3C CSS Spatial Navigation, UWP XYFocus, and focus-zones patterns from tvOS.

### Crate placement

Lives in `swissarmyhammer-focus/src/navigate.rs` as the **default `BeamNavStrategy::next` implementation** of the `NavStrategy` trait (trait declared by the refactor card `01KQ2E7RPBPJ8...`). Tests in `swissarmyhammer-focus/tests/navigate.rs`. The Tauri adapter in `kanban-app/src/commands.rs` is thin — it derives `WindowLabel` from `tauri::Window`, calls `SpatialState::navigate` (which delegates to the configured `NavStrategy`), and emits `focus-changed`.

### The core rule: nav happens *in* a layer

**Navigation never crosses a layer boundary.** The focused entry's `layer_key: LayerKey` is the hard boundary. Two windows open, inspector on one, dialog on the other — each is its own layer, nav in each stays put. Absolute.

### Three-rule beam search (leaf level)

Within a layer, beam search from a `Focusable` runs in priority order:

1. **Within-zone beam (container-first)**
   Candidates = entries where `candidate.layer_key == focused.layer_key && candidate.parent_zone == focused.parent_zone && candidate.is_focusable()`. Run beam test + Android scoring (`13 * major² + minor²`). If found, return `Some(candidate.moniker().clone())`.

2. **Cross-zone leaf fallback**
   Candidates = all `Focusable` entries with matching `layer_key`. Same beam + scoring. Makes `nav.right` across columns work naturally.

3. **No-op**
   Return `None`.

`FocusZone` entries are **not** candidates at leaf level.

### Zone-level nav

When the focused entry is a `FocusZone` (user drilled out), beam search runs against **sibling zones**:

- Candidates = entries where `candidate.layer_key == focused.layer_key && candidate.parent_zone == focused.parent_zone && candidate.is_zone()`.
- Leaves invisible at this level.

### API — newtyped signatures

```rust
// In swissarmyhammer-focus/src/navigate.rs
impl SpatialRegistry {
    pub fn navigate(&self, key: SpatialKey, direction: Direction) -> Option<Moniker> {
        let focused: &FocusScope = self.scope(&key)?;

        // 0. Override check (card 01KNQY1GQ9...) — returns Option<Moniker>
        if let Some(target) = self.check_override(focused, direction) { return target; }

        let layer: &LayerKey = focused.layer_key();

        match focused {
            FocusScope::Focusable(f) => {
                if let Some(m) = self.beam_in_zone(f, direction, layer) { return Some(m); }
                self.beam_all_leaves_in_layer(f, direction, layer)
            }
            FocusScope::Zone(z) => {
                self.beam_sibling_zones(z, direction, layer)
            }
        }
    }

    fn beam_in_zone(&self, from: &Focusable, dir: Direction, layer: &LayerKey) -> Option<Moniker>;
    fn beam_all_leaves_in_layer(&self, from: &Focusable, dir: Direction, layer: &LayerKey) -> Option<Moniker>;
    fn beam_sibling_zones(&self, from: &FocusZone, dir: Direction, layer: &LayerKey) -> Option<Moniker>;
}
```

Beam math operates on `Pixels` from `focus/types.rs`, not raw `f64`.

### Direction enum

```rust
pub enum Direction {
    Up, Down, Left, Right,
    First, Last,
    RowStart, RowEnd,
}
```

Drill-in / drill-out are **separate commands** (see card `01KPZS4RG0...`), not directions.

### React side

`broadcastNavCommand` becomes fire-and-forget: `invoke("spatial_navigate", { key, direction })`. Rust emits `FocusChangedEvent` with the new `SpatialKey` + `Moniker`; the claim registry from card `01KNM3YHHFJ3...` applies it.

### Subtasks
- [x] `swissarmyhammer-focus/src/navigate.rs`: `beam_test` + Android scoring on `Pixels`
- [x] `beam_in_zone` (rule 1)
- [x] `beam_all_leaves_in_layer` (rule 2)
- [x] `beam_sibling_zones` (zone-level)
- [x] Edge commands (First, Last, RowStart, RowEnd) with level-aware candidate sets
- [x] Override check as rule 0 (implemented in card `01KNQY1GQ9...`)
- [x] Tauri command `spatial_navigate` in `kanban-app/src/commands.rs` (thin adapter)

## Acceptance Criteria
- [x] Lives in `swissarmyhammer-focus/src/navigate.rs`
- [x] `navigate` signature is `(SpatialKey, Direction) -> Option<Moniker>`
- [x] All beam helpers return `Option<Moniker>`
- [x] Candidate filtering compares newtypes (`LayerKey` / `SpatialKey`), not raw strings
- [x] `navigate` never returns a `Moniker` from a different layer
- [x] Leaf arrow-nav: in-zone preferred; cross-zone leaf fallback fires when no in-zone
- [x] Zone arrow-nav: only sibling zones; leaves invisible
- [x] All 8 directions correct at both levels
- [x] Aligned candidates preferred over closer-but-diagonal (13:1)
- [x] `cargo test -p swissarmyhammer-focus` passes

## Tests (`swissarmyhammer-focus/tests/navigate.rs`)

Pure-Rust with synthetic rects using `Pixels(...)` constructors and `SpatialKey::from_string(...)` / `LayerKey::from_string(...)` / `Moniker::from_string(...)`. No raw strings. Headless pattern matching `tests/resolve_focused_column.rs`.

### Layer isolation (absolute)
- Nav never crosses `LayerKey` — card in window W invisible to inspector I inside W
- Parallel windows isolated — same rect coords in A and B never mix

### Rule 1: within-zone beam
- Focused title in card with sibling status leaf; `nav.down` → sibling status (same `parent_zone`)

### Rule 2: cross-zone leaf fallback
- Focused leaf with no in-zone down-neighbor → nearest leaf below in layer (even if across columns)

### Zone-level nav
- Focused on `col0: FocusZone`, `nav.right` → `col1` (sibling zone)
- Focused on `col0: FocusZone`, `nav.up` → `None` (no vertical sibling zones)
- `nav.right` does NOT return a leaf inside `col1` even if rect-wise nearest

### Inspector layer
- `pill_a` → `nav.right` → `pill_b` (in-zone)
- `label_1` → `nav.down` → `label_2` (rule 2 across field rows)
- Last leaf → `nav.down` → `None` (no layer escape)

### Realistic board
- 3 columns × N cards, each card a Zone with title + status leaves

### Edge commands
- `nav.first` / `nav.last` scope by level (Leaf stays in `parent_zone`; Zone scopes to sibling zones)

### Layer-boundary stress
- Dialog → Inspector → Window: focused in dialog sees only dialog entries

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.