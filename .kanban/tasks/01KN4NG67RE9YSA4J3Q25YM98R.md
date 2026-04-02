---
assignees:
- claude-code
depends_on:
- 01KN4NEVT8AVMVWX4WTHJG15FJ
position_column: todo
position_ordinal: '8180'
title: 6. Perspective tab bar + default perspective
---
## What

Add a tab bar component at the top of the view area showing perspectives for the current view kind, with a "+" button to create new ones.

**Files to create:**
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — tab bar component

**Files to modify:**
- `kanban-app/ui/src/App.tsx` — insert PerspectiveTabBar between NavBar and ActiveViewRenderer

**Approach:**
- Tab bar sits directly above the view content area (between the nav row and the board/grid)
- Filters perspectives by current view kind (e.g. show only "board" perspectives when board view is active)
- Each tab shows perspective name, active tab is highlighted
- "+" button at end creates a new perspective via `backendDispatch({ cmd: "perspective.save", args: { name: "Untitled", ... } })`
- When no perspectives exist for the current view kind, auto-create a "Default" perspective
- Tab context menu (right-click): rename, duplicate, delete
- Rename uses inline editing (contentEditable or small input)
- Delete dispatches `perspective.delete`
- Compact design — single row, doesn't consume much vertical space

**Default perspective logic:**
- On PerspectiveProvider mount, if `perspective.list` returns empty for current view kind, auto-create "Default" perspective via `perspective.save`
- Default perspective has no filter, no sort, no group — just the baseline view

## Acceptance Criteria
- [ ] Tab bar renders at top of view area with perspective names
- [ ] Active perspective tab is visually highlighted
- [ ] "+" button creates a new perspective and switches to it
- [ ] Default perspective auto-created when none exist
- [ ] Right-click context menu with rename and delete
- [ ] Tab click switches active perspective
- [ ] Tabs filtered by current view kind

## Tests
- [ ] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — renders tabs, click switches perspective, "+" creates new
- [ ] `pnpm test` from `kanban-app/ui/` passes