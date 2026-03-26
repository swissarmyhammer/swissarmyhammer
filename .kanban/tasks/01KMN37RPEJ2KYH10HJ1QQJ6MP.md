---
assignees:
- claude-code
position_column: todo
position_ordinal: c480
title: Fix mention pill focus bar — add left spacing and close gap to pill
---
## What

When a mention pill (tag, actor, etc.) is focused in the badge-list display or inspector, the focus indicator bar renders at `left: -0.5rem` (global default). Because pills sit in a `flex flex-wrap gap-1` container (`badge-list-display.tsx:65`), the bar overlaps the pill to the left. It also appears visually detached from the pill it belongs to.

### Root cause

`MentionPill` in `kanban-app/ui/src/components/mention-pill.tsx` wraps content in a `FocusScope` with `className="inline"` (line 108). When focused, the global `[data-focused]::before` rule (in `index.css:137`) positions the bar at `left: -0.5rem`. For inline pill elements in a tight flex layout with only `gap-1` (0.25rem) spacing, `-0.5rem` extends into the neighboring pill's space.

Two issues:
1. **Bar overlaps left neighbor** — the bar at `-0.5rem` extends 0.5rem left, but `gap-1` only provides 0.25rem of space
2. **Bar too far from its pill** — the gap between bar and pill text makes it unclear which pill is focused

### Fix

1. **Add a CSS class** for pill focus in `kanban-app/ui/src/index.css`:
   ```css
   .mention-pill-focus[data-focused]::before {
     left: -0.125rem;
   }
   ```
   This keeps the bar just outside the pill but within the flex gap, avoiding overlap with neighbors.

2. **Increase gap** in `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` from `gap-1` to `gap-1.5` to give more breathing room between pills when one is focused.

3. **Add the class** to the FocusScope in `kanban-app/ui/src/components/mention-pill.tsx`:
   ```tsx
   <FocusScope moniker={scopeMoniker} commands={commands} className="inline mention-pill-focus">
   ```

### Files to modify

- **Modify**: `kanban-app/ui/src/index.css` — add `.mention-pill-focus[data-focused]::before` rule
- **Modify**: `kanban-app/ui/src/components/mention-pill.tsx` — add `mention-pill-focus` class to FocusScope
- **Modify**: `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` — increase gap from `gap-1` to `gap-1.5`

## Acceptance Criteria

- [ ] Pill focus bar does not overlap the neighboring pill to the left
- [ ] Pill focus bar is visually close to its pill (not detached)
- [ ] Focus bars on card fields and column headers are unaffected
- [ ] Pills with no left neighbor (first pill in row) still show focus bar correctly

## Tests

- [ ] Visual verification — focus a tag pill in the inspector, bar should not overlap neighbor
- [ ] `pnpm vitest run` passes (no rendering regressions)
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — existing tests still pass">