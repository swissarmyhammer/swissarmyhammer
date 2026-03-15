---
position_column: done
position_ordinal: ffff9480
title: DataTable renderer — static grid
---
Build the DataTable component that renders entities as a table with columns from field definitions. Connects to useGrid for cursor/selection state. Read-only at this stage.

**Create `ui/src/components/data-table.tsx`:**
- [ ] Props: `entities: Entity[]`, `fields: FieldDef[]`, `cursor: {row, col}`, `selection: Set<number>`, `mode`
- [ ] Sticky `<thead>` with column headers from field names
- [ ] `<tbody>` rows from entities, each cell rendered via CellDispatch
- [ ] Focused cell (matching cursor) gets ring/border highlight
- [ ] Selected rows get background highlight
- [ ] Column widths from `field.width` or sensible defaults
- [ ] Empty state: "No tasks" message when entities is empty

**Update `ui/src/components/grid-view.tsx`:**
- [ ] Use `useSchema()` to get field definitions for entityType
- [ ] Use `useEntityStore()` to get entities
- [ ] Filter fieldNames through schema to get ordered FieldDef[]
- [ ] Initialize `useGrid({rowCount: entities.length, colCount: fields.length})`
- [ ] Render `<DataTable>` with all props wired up
- [ ] Table renders correctly with task data from entity store