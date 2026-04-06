---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe580
title: Fix grid cell click focus — EntityRow onClick overwrites cell focus
---
## What

After adding row-level `FocusScope` to DataTable rows (commit aab319854), clicking a grid cell no longer focuses that cell. The cursor doesn't move.

**Root cause:** `EntityRow` onClick (data-table.tsx:578-582) calls `setFocus(entityMk)` which sets entity focus to the row-level moniker (e.g. `task:t1`). The cell-level click handler `handleCellClick(di, ci)` calls `setFocus(cellMoniker)` (e.g. `task:t1.title`). Both fire because the click event bubbles from cell → row. The row handler fires AFTER the cell handler (bubble order), so `setFocus(entityMk)` overwrites `setFocus(cellMoniker)`.

**Fix:** Remove `setFocus(entityMk)` from `EntityRow` onClick. The cell click handlers already manage cursor focus. The row only needs `setFocus` in the context menu handler (to set entity focus before opening the menu) — that one stays.

**Files to modify:**
- `kanban-app/ui/src/components/data-table.tsx` — `EntityRow` onClick: remove `setFocus(entityMk)`, keep the interactive element guard but just return (or remove onClick entirely since cells handle it)

## Acceptance Criteria
- [ ] Clicking a grid cell moves the cursor to that cell
- [ ] Right-click still sets entity focus before opening context menu
- [ ] Double-click still dispatches inspect

## Tests
- [ ] Add test to `data-table.test.tsx`: click a cell, verify `onCellClick` callback fires with correct (row, col)
- [ ] `cd kanban-app/ui && pnpm vitest run src/components/data-table.test.tsx` — all pass
- [ ] Manual: grid view — click cells, verify cursor moves