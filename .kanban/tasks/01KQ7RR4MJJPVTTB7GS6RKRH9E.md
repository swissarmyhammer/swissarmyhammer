---
assignees:
- claude-code
depends_on:
- 01KQ7GWE9V2XKWYAQ0HCPDE0EZ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffcd80
project: spatial-nav
title: 'Fix: nav.up / nav.down from a focused card drops to no focus — direction-asymmetric registry bug'
---
## SUPERSEDED — see `01KQ7STZN3G5N2WB3FF4PM4DKX`

This card is subsumed by `01KQ7STZN3G5N2WB3FF4PM4DKX` ("Directional navigation from a focused card: all four directions, one ticket"). The user explicitly asked to unify per-direction tactical cards into a single ticket for card-directional-nav. This card's vertical-axis test cases migrate into the new file `kanban-app/ui/src/components/card-directional-nav.spatial.test.tsx`.

**Move this card to `done` once the new card lands, with a "subsumed by `01KQ7STZN3G5N2WB3FF4PM4DKX`" note.**

---

## Original description (preserved for context)

After the cross-column fix lands (`01KQ7GWE9V2XKWYAQ0HCPDE0EZ`), pressing left or right from a focused card moves between columns correctly. **But pressing up or down from a focused card drops focus to nothing** — no card in the same column gets the focus indicator. Side-to-side works, vertical does not.

The keymap is symmetric: `nav.up = k/ArrowUp`, `nav.down = j/ArrowDown`, `nav.left = h/ArrowLeft`, `nav.right = l/ArrowRight` — all four pass through the same `buildNavCommands` factory in `kanban-app/ui/src/components/app-shell.tsx` and call `spatial_navigate(focusedKey, direction)` with the same shape. The asymmetry must live in **what's focused** or **what gets registered for vertical-axis candidate selection**.

(Original suspect lists and full test spec preserved in earlier card revisions; all of that is now folded into `01KQ7STZN3G5N2WB3FF4PM4DKX`.)