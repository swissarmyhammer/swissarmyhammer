---
assignees:
- claude-code
depends_on:
- 01KQSEFZ8VQ67KFA0B4QE84Z2X
position_column: todo
position_ordinal: d680
project: spatial-nav
title: 'Rewrite swissarmyhammer-focus README: short, single-primitive, no FocusZone'
---
## What

Rewrite `swissarmyhammer-focus/README.md` as a one-page rules document.

The kernel has exactly this surface:

**One primitive:** `FocusScope` — has a rect, may have children, lives in a layer, identified by an FQM (path through the hierarchy).

**One boundary:** `Layer` — nav and drill never cross it.

**Eight operations:**

| Op | Rule |
|---|---|
| up / down / left / right | Geometric beam pick across all scopes in the layer in direction D. Half-plane + cross-axis overlap + Android beam score (`13*major² + minor²`). Tie: leaves over scopes-with-children. Empty → stay-put. |
| drill down | Focused scope's `last_focused` if live, else first child by (top, left). No children → stay-put. |
| drill up | Focused scope's parent. No parent → stay-put. |
| first sibling | First child of the focused scope's parent by (top, left). |
| last sibling | Last child of the focused scope's parent by (bottom, right). |

**Two invariants:**

- **No-silent-dropout** — every op returns an FQM. Stay-put = echo focused FQM.
- **Coordinate system** — all rects viewport-relative, sampled by `getBoundingClientRect()`. Mixing frames silently picks wrong neighbors.

**One non-feature:** scrolling. The kernel doesn't know about DOM scroll containers or virtualizers. The React layer scrolls on stay-put.

That's the whole document. ~80 lines max.

## Delete from current README

- "The sibling rule" (subsumed by geometric nav)
- "Why geometric (and not structural)" (history)
- "RowStart / RowEnd are deprecated aliases" (irrelevant after collapse)
- "Kind is not a filter" anti-pattern callout (subsumed)
- "Audit history" sections
- All cross-references to test files
- All ASCII tree diagrams beyond what one rule needs
- "Pinned by" / "See also" lists
- All `FocusZone` mentions (collapsed)

## Acceptance Criteria

- [ ] `wc -l swissarmyhammer-focus/README.md` < 100 lines
- [ ] `grep -c "FocusZone\|is_zone" swissarmyhammer-focus/README.md` == 0
- [ ] Document covers exactly the eight ops, two invariants, one non-feature above
- [ ] No section refers to test files, audits, or commit history

## Tests

- [ ] `wc -l swissarmyhammer-focus/README.md` < 100
- [ ] `grep -c "FocusZone\|is_zone" swissarmyhammer-focus/README.md` == 0

## Dependencies

Runs after FocusZone collapse (`01KQSEFZ8VQ67KFA0B4QE84Z2X`).

#spatial-nav-redesign