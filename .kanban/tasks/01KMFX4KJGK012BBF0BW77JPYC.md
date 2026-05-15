---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff980
title: Fix focus indicator position on entity cards (compact mode)
---
## What

The `[data-focused]::before` focus bar on entity cards (board view, compact mode) renders at `left: -0.5rem` relative to the `FocusHighlight` wrapper, which wraps the **entire card** including padding and grip handle. This places the bar outside the card border — far from the field icons.

In the inspector (full mode), `FocusHighlight` wraps each **field row** directly (`entity-inspector.tsx:208-228`), so the same `left: -0.5rem` correctly positions the bar just to the left of the icon. The entity card needs to match this alignment.

**Root cause**: In `entity-card.tsx:72`, `FocusScope` wraps the outer card div. The icon is nested deep inside: `FocusHighlight → card div (px-3) → GripVertical + fields div → per-field div → Icon`. The `::before` pseudo-element on `FocusHighlight` has no awareness of where the icon content starts.

**Files to modify:**
- `kanban-app/ui/src/components/entity-card.tsx` — restructure so the focus bar aligns with field icons, not the outer card edge
- `kanban-app/ui/src/index.css` — possibly adjust `[data-focused]::before` positioning if a scoped override is needed (lines 133-147)

**Approach**: Adjust the `::before` left offset for entity cards so it aligns with the icon area. Options:
1. Add a CSS class or data attribute to entity-card's `FocusScope` and override `::before { left }` to account for the grip handle + padding indent (~`left: 1.75rem`)
2. Or restructure the card layout so `FocusHighlight` wraps only the field content area (keeping `FocusScope` on the outer div for click/context-menu behavior)

Option 2 is cleaner but requires separating `FocusScope`'s visual indicator from its event-handling wrapper — check if `FocusHighlight` can be placed independently of `FocusScope`.

## Acceptance Criteria
- [ ] Focus indicator bar appears to the left of field icons on entity cards, matching the inspector's visual alignment
- [ ] Focus indicator still works on columns and inspector rows (no regression)
- [ ] Grip handle and info button are NOT visually inside the focus bar area

## Tests
- [ ] Visual test: click an entity card on the board — focus bar should align with the leftmost field icon, not the card border
- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` or `focus-scope.test.tsx` — verify `data-focused` attribute is still set correctly on card focus
- [ ] Run `cargo nextest run` — no backend regressions
- [ ] Run frontend tests — no component regressions