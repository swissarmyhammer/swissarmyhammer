---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: b780
title: Fix grid cursor reset to (0,0) after committing color picker edit
---
## What

When editing a color picker cell in the grid (e.g. tag color), committing the edit snaps the cursor to the upper-left cell (0,0) instead of staying on the edited cell. Likely pre-existing.

**Probable root cause:** The initial-focus effect in grid-view.tsx (lines 132-141) fires whenever `derivedCursor` is null AND `firstCellMoniker` changes. After a color edit commits, entity-field-changed triggers re-render → entities array changes → `cellMonikers` rebuilds → `firstCellMoniker` changes reference → effect re-fires → snaps to (0,0).

**Fix:** Guard the initial-focus effect with a ref so it only fires on true initial mount, not on every re-render where deps change.

```tsx
const hasInitialFocusRef = useRef(false);
useEffect(() => {
  if (!firstCellMoniker) return;
  if (hasInitialFocusRef.current) return;
  if (!derivedCursor) {
    setFocus(firstCellMoniker);
    hasInitialFocusRef.current = true;
  }
}, [firstCellMoniker, setFocus, derivedCursor]);
```

**Files to modify:**
- `kanban-app/ui/src/components/grid-view.tsx` — guard initial-focus effect with `hasInitialFocusRef`

## Acceptance Criteria
- [ ] Editing a color picker cell and committing keeps cursor on that cell
- [ ] Editing any cell type and committing keeps cursor on that cell
- [ ] First mount still focuses cell (0,0) when no cell has focus
- [ ] Switching to grid view from board view still focuses cell (0,0)

## Tests
- [ ] `cd kanban-app/ui && pnpm vitest run` — all pass
- [ ] Manual: grid view on tags — click color cell, pick color, verify cursor stays