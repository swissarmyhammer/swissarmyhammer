---
depends_on:
- 01KKP5XB7XW4SYF8DE2W1T1FVZ
position_column: done
position_ordinal: a280
title: Add search icon button to NavBar
---
## What
Add a search icon button in the NavBar toolbar so mouse users can click to launch search mode. Uses the `Search` icon from lucide-react.

**Files:** `kanban-app/ui/src/components/nav-bar.tsx`

**Approach:**
- Import `Search` from lucide-react
- Add a button next to the existing board info button that dispatches `app.search` via `useExecuteCommand()` from command scope
- Style consistently with the existing info button (ghost, muted foreground)

## Acceptance Criteria
- [ ] Search icon visible in the NavBar toolbar
- [ ] Clicking it opens the command palette in search mode
- [ ] Consistent styling with existing toolbar buttons

## Tests
- [ ] Manual: click search icon, palette opens in search mode