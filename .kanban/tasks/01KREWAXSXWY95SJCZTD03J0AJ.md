---
assignees:
- claude-code
position_column: todo
position_ordinal: aa80
title: GroupedBoardView doesn't virtualize — switching to group-by takes ~3 min on 2300 tasks
---
## What

On a board with 2300 tasks, switching the perspective group field (e.g. group-by Status) takes **roughly three minutes** before the new layout finishes rendering. The dispatch is not deadlocked — it eventually returns successfully — but the UX is "is this hung?"

## Confirmed diagnosis

The user nailed this:

> *"when i switch the group by to none which works great, it is instant"*

Ungrouped → instant. Grouped → 3 minutes. So:

- **Backend `perspective.group` dispatch is fine.** Same dispatch fires for set-to-none (which is instant via `perspective.clearGroup`, structurally identical) and is fast.
- **Frontend ungrouped layout is fine.** `<BoardView>` (without grouping) virtualizes correctly — only viewport-visible cards mount.
- **The bottleneck is exclusively `<GroupedBoardView>` rendering every card simultaneously.**

3 minutes ÷ 2300 cards ≈ **78ms per card**. That number is exactly what un-virtualized React rendering of a moderately-heavy card component looks like, applied serially to every card in the dataset. Each group's column renders its full task list eagerly, instead of using a fixed-height viewport with windowed scrolling.

The fix is structural: `<GroupedBoardView>` must virtualize cards per group, the same way the ungrouped view does. Each group's column is a fixed-height (one screen) scrollable region with only the viewport-visible cards plus a small over-scan buffer mounted at any time.

## Files to investigate

- `kanban-app/ui/src/components/grouped-board-view.tsx` — the offender. Walk its rendering structure; almost certainly it maps over `group.tasks` and renders `<TaskCard>` for every one, no virtualizer wrapping the inner list.
- `kanban-app/ui/src/components/board-view.tsx` — the working sibling. Look at how it virtualizes its column. Same pattern needs to apply per-group.
- `kanban-app/ui/src/components/sortable-column.tsx` / `column-view.tsx` — these wrap the columns in the ungrouped board; check whether they already provide the virtualizer that grouped-board-view is bypassing.
- `kanban-app/ui/src/components/data-table.tsx` — grid view's virtualizer for reference; TanStack `useVirtualizer` is already a workspace dep.

## Acceptance Criteria

- [ ] On a board with 2300 tasks, switching the group field renders the new layout in **under 200ms**. Measured with browser `performance.now()` spanning click handler to first paint of the regrouped layout.
- [ ] `<GroupedBoardView>` virtualizes its task cards per group. Render-count probe confirms only viewport-visible cards (plus a small over-scan buffer, e.g. 5–10) mount on the initial regroup, NOT all N tasks in the group.
- [ ] Each group's column is independently scrollable, mirroring the existing ungrouped column scrolling.
- [ ] Switching back to group-by-none stays instant (no regression on the working path).
- [ ] Dragging a card within and across groups still works (the drag-and-drop integration must coexist with the virtualizer; TanStack virtualizer + dnd-kit is a known-working combination, look at how the ungrouped board does it).
- [ ] Fix is captured in the implementation notes with concrete before/after timings.

## Tests

- [ ] Frontend regression `kanban-app/ui/src/components/grouped-board-view.virtualization.test.tsx` (new):
  - Render with a 2300-task fixture distributed across (say) 5 groups in a fixed-size viewport.
  - Assert the count of mounted `<TaskCard>` instances is bounded by `groups * (viewport_cards_per_group + 2 * overscan)`, NOT 2300.
  - Scroll one group; assert previously-mounted cards unmount and new ones mount as expected.
- [ ] Frontend regression `grouped-board-view.perf.test.tsx` (new):
  - Render with a 2300-task fixture, dispatch `perspective.group`, assert the regrouped frame paints within 200ms (`performance.now()` delta).
- [ ] Drag-and-drop regression: re-run the existing drag tests against the virtualized grouped view to confirm reorder, cross-group move, and sortable-column behavior still work. Reuse fixtures from `grouped-board-view.test.tsx`.

## Workflow

- **Profile first.** Open React DevTools profiler in the live app on the 2300-task board. Click group-by. Confirm: count of mounted `<TaskCard>` instances at completion is 2300 (or ~ the total task count). Capture that as the "before" measurement.
- Look at how `<BoardView>` / `<SortableColumn>` virtualizes its column (search for `useVirtualizer`, `IntersectionObserver`, or similar). The same pattern needs to apply inside each group rendered by `<GroupedBoardView>`. Don't invent a new virtualization mechanism; reuse the one the ungrouped path already uses.
- The fix is most likely **localized to `grouped-board-view.tsx`** — wrap the inner card list with the same virtualizer the ungrouped column uses. Each group becomes a fixed-height (one viewport-screen) container with windowed cards. Total cards mounted at any time stays bounded by the viewport, not by the dataset.
- After the fix, the "ungrouped" path is the **regression baseline** — keep that path's behavior identical so you don't accidentally break the path that was already fast. #command-driven-ui #perf