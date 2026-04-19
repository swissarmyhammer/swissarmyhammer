---
assignees:
- claude-code
position_column: todo
position_ordinal: ab80
title: 'Fix scroll containment: only the view area should scroll'
---
## What

The outermost `h-screen flex flex-col` div in `kanban-app/ui/src/App.tsx` (the div wrapping NavBar, ViewsContainer, and ModeIndicator) has no `overflow-hidden`. When view content (board columns, grid rows) is taller than the viewport, the browser document itself scrolls — causing the NavBar, LeftNav, PerspectiveTabBar, and ModeIndicator (keymap footer) to scroll off-screen rather than staying fixed.

Fix: add `overflow-hidden` to the outermost div in `App.tsx`:

```tsx
// kanban-app/ui/src/App.tsx
<div className="h-screen bg-background text-foreground flex flex-col overflow-hidden">
```

The inner scroll containers are already correct and must remain unchanged:
- `ColumnView` task list: `flex-1 overflow-y-auto` (vertical column scrolling)
- `BoardView` scroll container: `overflow-x-auto` (horizontal column scrolling)
- `GroupedBoardView`: `overflow-y-auto` (grouped sections)
- `DataTable`: `overflow-auto` (grid view)

**Files to modify**: `kanban-app/ui/src/App.tsx` only.

## Acceptance Criteria

- [ ] The NavBar (`<header>` in `nav-bar.tsx`) remains visible and does not scroll out of view when board content is taller than the screen
- [ ] The PerspectiveTabBar (rendered by `PerspectivesContainer`) remains visible and fixed at the top of the content area
- [ ] The LeftNav sidebar remains fixed on the left
- [ ] The ModeIndicator (vim keymap footer, rendered by `mode-indicator.tsx`) remains pinned at the bottom
- [ ] The view area (board columns / grid rows) scrolls independently within its container
- [ ] No new `overflow-hidden` is added to inner view components (they already have correct overflow)

## Tests

- [ ] Add a browser/visual test in `kanban-app/ui/src/` that renders a board with enough columns/tasks to overflow the viewport and asserts that the NavBar element is not scrolled off screen (i.e., its `getBoundingClientRect().top` stays at 0)
- [ ] Run `pnpm test` in `kanban-app/ui/` and confirm all existing tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.