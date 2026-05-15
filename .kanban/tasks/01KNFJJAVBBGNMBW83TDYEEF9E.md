---
assignees:
- claude-code
depends_on:
- 01KNFJGP4Y3YJBK3BKZDSNJ9AZ
- 01KNFJH9W9Z4XRJGQ41KV2904W
position_column: done
position_ordinal: ffffffffffffffffffffffffffcc80
title: 'GroupedBoardView: collapsible group sections, each containing a column layout'
---
## What

Create a `GroupedBoardView` component that, when a group field is active, renders a vertical stack of collapsible sections — each section labeled with the group value and containing a full horizontal column layout. When no grouping is active, render the existing `BoardView` unchanged.

### Files to create/modify

1. **Create `kanban-app/ui/src/components/grouped-board-view.tsx`**:
   ```
   GroupedBoardView({ board, tasks })
     - Reads `groupField` from `useActivePerspective()`
     - If no groupField: renders `<BoardView board={board} tasks={tasks} />`
     - If groupField: calls `computeGroups(tasks, groupField, fieldDefs)`
       then renders a vertical scrolling list of `<GroupSection>` components
   ```

2. **Create `kanban-app/ui/src/components/group-section.tsx`**:
   ```
   GroupSection({ bucket, board, groupField, collapsed, onToggle })
     - Collapsible header: chevron + group label + task count badge
     - When expanded: renders a `<BoardView>` with only the bucket's tasks
     - Collapse state is local (useState), default expanded
   ```

3. **Modify the parent that currently renders `<BoardView>`** — swap in `<GroupedBoardView>` so grouping is activated by the perspective's group field. Find where `BoardView` is rendered (likely in `PerspectiveContainer` or a views container) and replace with `GroupedBoardView`.

### Layout design

```
┌─ Group: "bug" ────────────────────── [▼] 5 tasks ─┐
│  ┌─ To Do ─┐  ┌─ Doing ─┐  ┌─ Done ─┐            │
│  │  card    │  │  card   │  │  card  │            │
│  │  card    │  │         │  │  card  │            │
│  └─────────┘  └─────────┘  │  card  │            │
│                             └────────┘            │
├─ Group: "feature" ────────────────── [▼] 3 tasks ─┤
│  ┌─ To Do ─┐  ┌─ Doing ─┐  ┌─ Done ─┐            │
│  │  card    │  │  card   │  │        │            │
│  └─────────┘  │  card   │  │        │            │
│               └─────────┘  └────────┘            │
├─ (ungrouped) ─────────────────────── [▼] 1 task ──┤
│  ...                                               │
└────────────────────────────────────────────────────┘
```

Each `BoardView` inside a group section receives the same `board` (columns) but only the tasks for that group. The existing `BoardView` already handles column layout, DnD, focus, etc.

### DnD integration

Each group section's `BoardView` must pass `groupValue` through to its `ColumnView` → `computeDropZones` calls so that drop zones carry the group context. This means:
- `BoardView` gets an optional `groupValue?: string` prop
- When present, `ColumnView` passes it to `computeDropZones(taskIds, columnId, groupValue)`
- `persistMove` in `BoardView` reads `descriptor.groupValue` and dispatches an `entity.update_field` call when the group changes (next card handles the dispatch logic)

### Column headers

Group sections share the same columns. Column headers (name, badge, add button) should appear in EACH group section so each section looks like a full board. The badge should show the count for that group's tasks in that column, not the total.

## Acceptance Criteria

- [ ] When no groupField is active, board renders identically to before (no visual change)
- [ ] When groupField is active, board shows collapsible sections per group value
- [ ] Each section has a header with group label, task count, and collapse chevron
- [ ] Each section contains a full column layout with that group's tasks
- [ ] Sections are collapsible — clicking the header toggles the section
- [ ] Ungrouped tasks appear in a \"(ungrouped)\" section at the bottom
- [ ] Drop zones within each section carry the group's value
- [ ] Existing keyboard navigation works within each section

## Tests

- [ ] `kanban-app/ui/src/components/grouped-board-view.test.tsx` — renders BoardView directly when no groupField
- [ ] `kanban-app/ui/src/components/grouped-board-view.test.tsx` — renders group sections when groupField is active
- [ ] `kanban-app/ui/src/components/grouped-board-view.test.tsx` — correct task count per section
- [ ] `kanban-app/ui/src/components/group-section.test.tsx` — collapse/expand toggle
- [ ] `npm test` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.">