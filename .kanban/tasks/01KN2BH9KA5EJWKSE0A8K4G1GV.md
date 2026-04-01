---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa380
title: 'Perf: virtualize column rendering with fixed-height cards and drop zones'
---
## What

Dragging a card over a column with many tasks causes jank from two compounding issues:
1. `DropZone` changes height on hover (6px→24px), triggering layout reflow of every card below
2. All N cards + N+1 drop zones are in the DOM regardless of visibility

### Fix — two parts

**Part 1: Fixed-height drop zones (no reflow)**

`kanban-app/ui/src/components/drop-zone.tsx` — remove the height toggle (`height: isOver ? 24 : 6`). Drop zones are a fixed height. The visual indicator toggles visibility (`opacity`), not size. Each zone keeps its own `onDragOver`/`onDragEnter`/`onDragLeave` — no coordinate math, no column-level hit-testing. The zone knows when it's hovered and carries its own `DropZoneDescriptor`.

**Part 2: Virtualize the column list**

`kanban-app/ui/src/components/column-view.tsx` — use `@tanstack/react-virtual` (`useVirtualizer`) to render only visible items. Each item in the virtual list is a (DropZone + Card) pair at a fixed height. The virtualizer manages the scroll container and only mounts DOM nodes for visible items + overscan.

Since cards and zones are both fixed height, the virtualizer can compute positions exactly — no measurement needed. Drop zones retain their own drag event handlers and `DropZoneDescriptor` data, so drop behavior is unchanged. The virtualizer just controls which ones are in the DOM.

This is a Tauri app (Chromium), so `@tanstack/react-virtual` works perfectly.

### Files to modify
- `kanban-app/ui/src/components/drop-zone.tsx` — fixed height, opacity-based indicator
- `kanban-app/ui/src/components/column-view.tsx` — add `useVirtualizer` for the card+zone list
- `kanban-app/ui/package.json` — add `@tanstack/react-virtual` dependency

## Acceptance Criteria
- [ ] Drop zone height does not change during drag-over
- [ ] Only visible cards + zones are in the DOM (check with DevTools)
- [ ] Dragging over a column with 50+ cards is smooth
- [ ] Drop still works correctly — card lands at the indicated position
- [ ] Drop zones still handle their own drag events (no column-level hit-testing)
- [ ] Auto-scroll near column edges still works during drag

## Tests
- [ ] `kanban-app/ui/src/components/drop-zone.test.tsx` — existing tests pass
- [ ] `kanban-app/ui/src/components/board-drag-drop.test.tsx` — zone computation unchanged
- [ ] `npx vitest run` in `kanban-app/ui` — no regressions
- [ ] Manual: 50+ card column — smooth drag, correct drop positions, auto-scroll works