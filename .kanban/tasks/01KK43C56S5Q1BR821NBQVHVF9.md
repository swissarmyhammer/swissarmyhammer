---
position_column: done
position_ordinal: ffff9f80
title: Integrate TanStack Table for sorting and grouping
---
Replace hand-rolled sorting in DataTable with TanStack Table (`@tanstack/react-table`). Add grouping support.

Current state:
- `data-table.tsx` has custom `compareValues()` and `sortedRows` useMemo for sorting
- No grouping capability
- `useGrid` manages cursor/mode/selection — this stays as-is (vim layer on top)

Integration approach:
- Use `useReactTable()` from TanStack with `getCoreRowModel`, `getSortedRowModel`, `getGroupedRowModel`
- Map `DataTableColumn[]` to TanStack `ColumnDef[]` with `accessorFn: (row) => row.fields[field.name]`
- Custom cell renderer via TanStack's `cell` column option — calls shared display components
- TanStack handles sort state and grouping state; remove custom `compareValues()` and `sortedRows`
- Keep `useGrid` cursor navigation — it operates on row indices from TanStack's `getRowModel().rows`
- Header click toggles TanStack sorting (replaces custom `handleHeaderClick`)
- Add column grouping via view YAML config (`group_by` field?)

Files to modify:
- `data-table.tsx` — replace sort logic with TanStack, add grouping
- `grid-view.tsx` — pass TanStack table instance, adapt useGrid rowCount to flattened row model

Files to add:
- None — TanStack is used inline in DataTable

- [ ] Install `@tanstack/react-table`
- [ ] Refactor DataTable to use `useReactTable()` with sorted and grouped row models
- [ ] Remove custom `compareValues()` and sort state from DataTable
- [ ] Map DataTableColumn to TanStack ColumnDef with existing cell display components
- [ ] Add grouping support (group by column via header menu or config)
- [ ] Ensure useGrid cursor navigation still works over TanStack's row model
- [ ] Ensure inline editing still works (renderEditor for cursor cell)
- [ ] Run tests