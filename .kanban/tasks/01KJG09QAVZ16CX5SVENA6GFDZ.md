---
title: Build kanban column view in frontend
position:
  column: done
  ordinal: a3^V
---
Build the visual kanban board display in the React frontend. Columns display left-to-right, filling vertical space, with a clean minimal design.

**Design spec:**
- Columns arranged horizontally, filling the full width equally
- Each column has a centered title at the top
- Column separator (thin vertical line between columns) instead of bordered columns — less visual noise
- Columns fill all available vertical space below the nav bar
- Tasks displayed as cards within their column, ordered by ordinal
- No drag-and-drop yet — just display

**New component: BoardView** (ui/src/components/board-view.tsx):
- Takes board (with columns) and tasks as props
- Renders a flex row of columns, each taking equal width (flex-1)
- Between columns: a thin vertical separator line (border-r or a Separator component)
- No left/right borders on outermost columns

**New component: ColumnView** (ui/src/components/column-view.tsx):
- Column header: centered title text, muted color, padded
- Below header: scrollable area for task cards
- flex-1 to fill vertical space

**New component: TaskCard** (ui/src/components/task-card.tsx):
- Compact card showing task title
- Maybe subtle background, rounded corners
- Show tag colors as small dots or pills if present
- Show dependency indicator if blocked

**App.tsx changes:**
- Fetch tasks in addition to board data (already fetching via list_tasks)
- Pass board + tasks to BoardView
- Replace placeholder text with BoardView

**Types:**
- Board already has columns[] and tasks interface exists
- May need to group tasks by column for efficient rendering

Files: ui/src/components/board-view.tsx (new), column-view.tsx (new), task-card.tsx (new), App.tsx (update)
Verify: cargo tauri dev shows columns left-to-right with task cards in the correct columns