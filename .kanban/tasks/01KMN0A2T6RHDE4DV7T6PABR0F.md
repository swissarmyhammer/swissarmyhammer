---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9b80
title: Fix board horizontal scroll pushing NavBar and LeftNav off screen
---
## What

When horizontally scrolling the board view, the NavBar (top toolbar) and LeftNav (left sidebar) scroll away with the content. They should be fixed in place — only the board content area should scroll.

### Root cause

In `kanban-app/ui/src/App.tsx:555`, the layout is:

```tsx
<div className=\"flex-1 flex min-h-0\">
  <LeftNav />
  <ActiveViewRenderer ... />  <!-- renders BoardView -->
</div>
```

`BoardView` (`board-view.tsx:585`) has `overflow-x-auto` on its scroll container, but the `ActiveViewRenderer` wrapper (or `BoardView`'s `FocusScope` root) doesn't constrain its width. Without `min-w-0` or `overflow: hidden` on the flex child containing the board, the flex item expands to fit its content, pushing the entire flex row wider than the viewport.

The NavBar above (`App.tsx:544`) is in the same `flex-col` parent as the flex row — if the row overflows, the column-level layout can be affected too.

### Fix

Add `min-w-0 overflow-hidden` to the flex child that contains `ActiveViewRenderer` so the board's horizontal scroll is contained within that well. The LeftNav and NavBar stay fixed.

Specifically:
1. In `App.tsx:555`, the `ActiveViewRenderer` needs to be wrapped (or the wrapper needs `min-w-0 overflow-hidden`) so its content doesn't push the flex parent wider.
2. Alternatively, add `min-w-0` to the `FocusScope` root in `board-view.tsx` — but the App-level fix is more correct since it applies to all view types.

### Files to modify

- **Modify**: `kanban-app/ui/src/App.tsx` — add `min-w-0 overflow-hidden` to the flex child containing `ActiveViewRenderer`

## Acceptance Criteria

- [ ] Horizontal board scroll does not move LeftNav or NavBar
- [ ] LeftNav remains pinned to the left edge during horizontal scroll
- [ ] NavBar remains pinned to the top during horizontal scroll
- [ ] Grid view still renders correctly (no clipping)
- [ ] Board columns are still horizontally scrollable within the content well

## Tests

- [ ] Visual: scroll board horizontally, verify NavBar and LeftNav stay fixed
- [ ] `pnpm vitest run` passes