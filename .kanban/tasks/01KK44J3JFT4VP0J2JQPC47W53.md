---
position_column: done
position_ordinal: ffffd880
title: 'data-table: cursor row index ri counts group header rows'
---
In data-table.tsx, `flatRows` from TanStack includes both group header rows and data rows. The `ri` index from `flatRows.map((row, ri) => ...)` is used for `grid.cursor.row` comparison, but group header rows increment `ri` without being real data rows. This means cursor highlighting (`ri === grid.cursor.row`) and `grid.setCursor(ri, ...)` will be off when grouping is active -- the grid hook tracks data-row indices but `ri` includes group rows.\n\nThis is latent because grouping is not yet externally enabled, but will break as soon as `groupingProp` is used.