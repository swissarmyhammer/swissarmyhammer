---
assignees:
- claude-code
position_column: review
position_ordinal: '8180'
title: Fix kanban board column overlap on narrow viewports
---
**Bug**: When the viewport is narrow, kanban board columns overlap each other instead of becoming horizontally scrollable.

**Root cause**: `SortableColumn` (the flex item rendered by `BoardColumnStrip` inside the `overflow-x-auto` scroll container) has `flex flex-1 min-w-[14em] max-w-[60em]` — no `shrink-0`. With `flex-1` (`flex: 1 1 0%`) and an explicit `min-width: 14em`, the SortableColumn flex item shrinks below the inner ColumnView's `min-w-[24em]`, so the inner column's content visually overflows into the neighboring column's flex slot.

The inner `ColumnView` FocusScope already has `min-w-[24em] max-w-[48em] shrink-0`, but that does not help because the parent flex item (SortableColumn) is the one that compresses.

**Fix**: Make SortableColumn `shrink-0` and align its width bounds with the inner column (`min-w-[24em]`, `max-w-[48em]`) so the flex item itself is what holds the width.

**Files**:
- kanban-app/ui/src/components/sortable-column.tsx — outer wrapper className

**Acceptance Criteria**:
- Narrowing the window causes the column strip to overflow horizontally with a scrollbar (no overlap).
- Wide window: layout looks identical to before.
- The existing `app-layout.test.tsx` "each column FocusScope carries shrink-0 plus min-w-[24em]/max-w-[48em]" test stays green.
- A new test in `app-layout.test.tsx` (or sortable-column-specific test) asserts SortableColumn carries `shrink-0` and the matching width bounds, so the same regression cannot recur.

**Tests**:
- `pnpm -C kanban-app/ui exec tsc --noEmit` clean
- `pnpm -C kanban-app/ui exec vitest run` for the column-width / app-layout suites stays green
- New regression test asserts `SortableColumn` outer wrapper has `shrink-0` plus a `min-w-[24em]` floor

**Out of scope**: Don't touch nav-bar.tsx, data-table.tsx, focus-scope.tsx (parallel work in flight).