---
assignees:
- claude-code
position_column: todo
position_ordinal: c180
project: spatial-nav
title: 'Spatial nav: drop same-kind filter at iter 0 — exhaust in-zone siblings of any kind before escalating'
---
## What

Inside a card the spatial graph today is `[drag-handle leaf] [title <Field> zone] [inspect leaf]` horizontally, then a column of field zones (e.g. `[tags] [add-tags]`) below. Two user-visible bugs from this layout:

  - **Right from drag jumps over title to inspect.** Expected: title.
  - **Left from inspect jumps over title to drag.** Expected: title.
  - **Down from drag (or inspect) leaves the card and lands on the next task.** Expected: the next field zone in the same card (e.g. the tags / add-tags row).

### Root cause

`swissarmyhammer-focus/src/navigate.rs::beam_among_siblings` (lines 385–408) applies a **same-kind filter** at iter 0:

```rust
if s.is_zone() != expect_zone {
    return None;
}
if s.parent_zone() == from_parent && s.fq() != from_fq {
    Some(...)
}
```

So when the focused entry is `card.drag-handle:{id}` (a leaf scope, parent_zone = `task:{id}`), iter 0 only finds OTHER LEAVES with the same parent_zone. The title `<Field>` is a Zone — it gets filtered out at iter 0. Inspect (also a leaf) wins. Down has no leaf candidate below in the card; the cascade escalates to iter 1 / drill-out and lands on the next card.

The justification documented in the module docs (lines 66–76) was historical:

> "a `<Field>` zone mounted inside a `<FocusScope>` card body inherits the card's enclosing `parent_zone` (the column), so the field zone and the card leaf are sibling-registered…"

That justification is **stale**. After the scope-is-leaf migration, cards are `<FocusZone>`s (`entity-card.tsx::EntityCard` line 106 — `<FocusZone moniker={asSegment(entity.moniker)}>`), so a Field zone inside a card now has `parent_zone = task:{id}` (the card), NOT the column. Same-kind filtering no longer prevents cross-card spillover; it just blocks the user from reaching the title.

### User's mental model (the algorithm we want)

> "When I am in a zone, I should navigate options in that zone. Pushing down on the bottom-most child of my containing zone should then go to the next peer zone. Down — anything below me in my parent zone, go there; nothing below me, next zone below my parent."

Translation:

  - **Iter 0 — any-kind, in-zone**: candidates are ANY registered scope (leaf OR zone) sharing my `parent_zone`, geometrically in `direction`. Pick best by Android beam score.
  - **Iter 1 — same-kind zones, parent's level**: only when iter 0 finds nothing, escalate. Iter 1 stays as-is (peer zones of my parent zone, sharing parent's parent — same-kind on zones because the parent IS a zone).
  - **Drill-out fallback**: unchanged — return parent zone's FQM if iter 1 also misses.

### Approach

Edit `swissarmyhammer-focus/src/navigate.rs` only:

1. In `beam_among_siblings` (lines 385–408), remove the `expect_zone` parameter and the kind filter inside the closure. The function becomes "any sibling sharing `from_parent`, in `layer`, scored by direction."
2. In `cardinal_cascade` (lines 276–340), the iter-0 call (lines 287–297) drops the `focused_is_zone` argument. The iter-1 call (lines 325–335) keeps `expect_zone = true` (parent is always a zone, peer zones of parent stay zone-only). Refactor: keep `beam_among_siblings` for iter 1, add a sister `beam_among_in_zone_any_kind` for iter 0 — clearer than threading an `Option<bool>` for kind filtering.
3. Update the module docstring (lines 40–76) to describe the new contract: iter 0 is any-kind in-zone; iter 1 keeps same-kind. Delete the stale "field zone inherits the column's parent_zone" justification — it predates the card-as-zone migration.
4. `edge_command` (lines 415–438) keeps its same-kind filter — `First` / `Last` / `RowStart` / `RowEnd` are level-bounded "first/last among siblings of my kind," which is the right semantics for those keys.

### Test ripple

Two existing tests pin the OLD behavior and must be updated:

  - `swissarmyhammer-focus/tests/card_directional_nav.rs::up_from_t1a_drills_out_to_column_zone` (lines 113–125). Today this expects `up` from the top card in column TODO to drill out to `column:TODO`. Under the new algorithm, the column-name `<FocusZone>` (geometrically above the card, sharing parent_zone = column TODO) is now a valid iter-0 candidate. Update the expected target to `column_name_zone_fq(0)` (or whatever the column-name zone's fixture FQM is — see `tests/column_header_arrow_nav.rs` for the symmetric case). Update the doc-comment to reflect the new contract.
  - `swissarmyhammer-focus/tests/column_header_arrow_nav.rs` — review any cases that rely on the same-kind filter at iter 0; flip expectations the same way. (Most of that file should still pass: it tests Down FROM the column header zone, where iter 0 already has zones as same-kind candidates.)

Other test files to scan but probably unaffected: `navigate.rs`, `unified_trajectories.rs`, `inspector_field_nav.rs`, `navbar_arrow_nav.rs`, `perspective_bar_arrow_nav.rs`. These either test edge commands, drill, or layouts where same-kind filter doesn't bite.

## Acceptance Criteria
- [ ] `swissarmyhammer-focus/src/navigate.rs::beam_among_siblings` (or its iter-0 successor) no longer filters by `is_zone`. Iter 0 considers any-kind scopes sharing the focused entry's `parent_zone`.
- [ ] Iter 1 (`beam_among_siblings` for parent peers) still filters to zones — peer-zone navigation stays clean.
- [ ] `edge_command` still filters by same-kind — `First`/`Last`/`RowStart`/`RowEnd` semantics unchanged.
- [ ] Module docstring (`navigate.rs` lines 40–76) is rewritten to describe the new "iter 0 = any kind in-zone" rule; the stale field-zone-inherits-column justification is removed.
- [ ] In a card with `[drag-handle leaf] [title field zone] [inspect leaf]` plus a vertical stack of field zones below: `Right` from drag lands on title; `Left` from inspect lands on title; `Down` from drag (or inspect) lands on the next field zone in the card (NOT the next card).
- [ ] Existing test `up_from_t1a_drills_out_to_column_zone` (`tests/card_directional_nav.rs` line 113) is updated to expect the column-name zone instead of the column zone, with a refreshed doc-comment explaining the new contract.

## Tests
- [ ] Add `swissarmyhammer-focus/tests/in_zone_any_kind_first.rs`. Build a fixture mirroring the card layout: a card zone with three children — a leaf at x=0 (drag-handle), a child zone at x=10 (title field), a leaf at x=20 (inspect) — plus a second child zone at y=20 (tags row). Assert:
  - `Right` from the leaf at x=0 returns the title-field zone's FQM (NOT the inspect leaf).
  - `Left` from the leaf at x=20 returns the title-field zone's FQM (NOT the drag leaf).
  - `Down` from the leaf at x=0 returns the tags-row zone's FQM (NOT escalation to a peer of the card).
  - `Down` from the inspect leaf at x=20 returns the tags-row zone's FQM (NOT escalation).
  - `Down` from the bottom-most field zone in the card escalates to iter 1 — returns the parent's peer zone (or drill-out to the card itself if no peer).
- [ ] Update `swissarmyhammer-focus/tests/card_directional_nav.rs::up_from_t1a_drills_out_to_column_zone` per the **Test ripple** section.
- [ ] Add `kanban-app/ui/src/components/entity-card.in-zone-nav.spatial.test.tsx`: mount a real `<EntityCard>` in the spatial-nav stack, seed focus on `card.inspect:{id}`, dispatch a synthetic keydown `ArrowLeft`, assert the focused FQM after the kernel emits `focus-changed` is the title field zone's FQM (NOT the drag-handle leaf, which the scope-is-leaf companion task is removing anyway). Same for `ArrowDown` from `card.inspect:{id}` — assert focus lands on a field zone inside the same card.
- [ ] `cargo nextest run -p swissarmyhammer-focus` passes including all updated tests.
- [ ] `cd kanban-app/ui && pnpm vitest run src/components/entity-card src/components/board-view src/components/column-view` passes.

## Workflow
- Use `/tdd` — write the new Rust integration test (`in_zone_any_kind_first.rs`) and the React spatial test first; watch them fail; flip the iter-0 filter; update the existing card-directional-nav assertion; confirm green.
- Coordinate with `01KQM9478XFMCBBWHQN6ARE524` (drag-handle FocusScope removal): once that lands, the "Right from drag" assertion in this task's tests becomes moot for the drag handle specifically — but the underlying iter-0 algorithm change is still validated by the inspect-leaf cases and by the synthetic Rust fixture, which doesn't depend on the production card's drag-handle leaf existing.
