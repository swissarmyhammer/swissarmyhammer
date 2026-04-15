---
assignees:
- claude-code
position_column: todo
position_ordinal: 7d80
project: task-card-fields
title: Fix horizontal scroll leaking past content area to app chrome
---
## What

The outer app shell allows horizontal overflow from the content area to scroll the entire page, moving the toolbar (NavBar), perspective tab bar, left nav, and mode footer along with the content. These chrome elements must stay fixed; only the content view area should scroll, and each view should own its own scrolling (e.g., the board has each column scroll vertically, and the column strip scrolls horizontally).

### Root cause

The `min-w-0` chain is broken between the viewport and the content container at `kanban-app/ui/src/App.tsx:61`. When the board has more columns than fit (each column has `min-w-[20em]` at `kanban-app/ui/src/components/column-view.tsx:423`), the intrinsic content size propagates up through flex parents that lack `min-w-0`, pushing the whole layout wider than the viewport so `html`/`body` scrolls horizontally.

Offending containers (all lack `min-w-0` or horizontal overflow clipping):
- `kanban-app/ui/src/App.tsx:50` — root `<div className="h-screen bg-background text-foreground flex flex-col">` has no `overflow-hidden`
- `kanban-app/ui/src/components/views-container.tsx:52` — `<div className="flex-1 flex min-h-0">` (flex row around LeftNav + perspectives) lacks `min-w-0` on the row items
- `kanban-app/ui/src/components/perspectives-container.tsx:35` — `<div className="flex flex-col flex-1 min-h-0">` lacks `min-w-0`
- `kanban-app/ui/src/components/board-view.tsx:565` — scroll container `<div className="flex flex-1 min-h-0 overflow-x-auto pl-2">` lacks `min-w-0`

Only `App.tsx:61` correctly uses `flex-1 min-w-0 overflow-hidden flex flex-col`, but its effect is defeated because ancestors above it can still expand to fit content width.

### Fix

1. `kanban-app/ui/src/App.tsx:50` — add `overflow-hidden` to the root container as a catch-all barrier so no descendant can ever push `html`/`body` wider than the viewport. Final: `h-screen bg-background text-foreground flex flex-col overflow-hidden`.
2. `kanban-app/ui/src/components/views-container.tsx:52` — add `min-w-0` so the flex-1 perspectives child cannot be pushed wider than its share. Final: `flex-1 flex min-h-0 min-w-0`.
3. `kanban-app/ui/src/components/perspectives-container.tsx:35` — add `min-w-0` so the content container below cannot push this column wider. Final: `flex flex-col flex-1 min-h-0 min-w-0`.
4. `kanban-app/ui/src/components/board-view.tsx:565` — add `min-w-0` to the `scrollContainerRef` div so it shrinks to its flex share and lets `overflow-x-auto` do the scrolling. Final: `flex flex-1 min-h-0 min-w-0 overflow-x-auto pl-2`.

Do not change the per-column vertical scroll at `kanban-app/ui/src/components/column-view.tsx:545` (`overflow-y-auto`) — that is the correct "view controls its own scrolling" behavior the user wants preserved. Likewise preserve `GridView`'s internal `DataTable` scrolling at `kanban-app/ui/src/components/data-table.tsx:233` (`flex-1 overflow-auto min-h-0`) and `GroupedBoardView`'s vertical scroll at `kanban-app/ui/src/components/grouped-board-view.tsx:58`.

### Subtasks
- [ ] Add a failing layout test (see Tests section) that mounts App with columns overflowing a narrow viewport and asserts `document.body.scrollWidth === document.body.clientWidth`.
- [ ] Apply the four CSS class changes above.
- [ ] Verify NavBar, PerspectiveTabBar, LeftNav, and ModeIndicator remain at fixed viewport positions while BoardView's column strip scrolls horizontally.
- [ ] Verify GridView still scrolls internally, not at the page level.
- [ ] Run the UI test suite and typecheck.

## Acceptance Criteria
- [ ] With the board open and enough columns to overflow the viewport, `document.body.scrollWidth` equals `document.body.clientWidth` (no page-level horizontal scroll).
- [ ] NavBar (`kanban-app/ui/src/components/nav-bar.tsx`), PerspectiveTabBar (`kanban-app/ui/src/components/perspective-tab-bar.tsx:195`), LeftNav (`kanban-app/ui/src/components/left-nav.tsx:35`), and ModeIndicator (`kanban-app/ui/src/components/mode-indicator.tsx:27`) bounding rects remain stable (same `left`/`right` in `getBoundingClientRect`) before and after horizontally scrolling the board.
- [ ] BoardView's `scrollContainerRef` at `board-view.tsx:565` has `scrollWidth > clientWidth` when columns overflow, and programmatically setting `scrollLeft` changes the visible column offset without moving any chrome element.
- [ ] GridView's DataTable continues to scroll internally (its `flex-1 overflow-auto min-h-0` div, not the page).
- [ ] Per-column vertical scrolling inside columns is unchanged.
- [ ] `cd kanban-app/ui && npm run typecheck` passes.

## Tests
- [ ] New test file: `kanban-app/ui/src/components/app-layout.test.tsx` — render `<App />` inside a container constrained to 800px width with a board containing 8+ columns (each `min-w-[20em]` = 320px), then assert:
  - `document.body.scrollWidth === document.body.clientWidth` (no body-level horizontal scroll)
  - The board's scroll container (query by the DnD sortable wrapper) has `scrollWidth > clientWidth`
  - `screen.getByRole('banner')` (NavBar) `getBoundingClientRect().left === 0` before and after calling `scrollContainer.scrollTo({ left: 200 })`
- [ ] Extend `kanban-app/ui/src/components/board-view.test.tsx` — add a test that verifies the `scrollContainerRef` div has the classes `min-w-0` and `overflow-x-auto`, and that with wide column content `scrollWidth > clientWidth`.
- [ ] Test command: `cd kanban-app/ui && npm test -- app-layout board-view` — all tests green.
- [ ] Full check: `cd kanban-app/ui && npm run typecheck && npm test` — no type errors, all tests green.
- [ ] Manual verification: `cd kanban-app && cargo tauri dev`, open a board with many columns, resize window narrow; confirm NavBar / tab bar / left nav / mode footer stay put while only the column strip scrolls.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.