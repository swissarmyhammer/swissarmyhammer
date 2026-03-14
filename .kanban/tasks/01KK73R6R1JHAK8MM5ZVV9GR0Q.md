---
position_column: done
position_ordinal: v6
title: 'Bug: Column min-width too large, causes premature horizontal scroll'
---
The board layout breaks with horizontal scroll appearing too soon. The minimum width of columns is too high — should be around 24em.

Key files: column-view.tsx or sortable-column.tsx (wherever min-width is set on columns)