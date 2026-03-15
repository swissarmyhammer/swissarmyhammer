---
position_column: done
position_ordinal: ffffa080
title: 'data-table: onContextMenu has identical if/else branches'
---
In data-table.tsx lines 149-155, the onContextMenu handler for headers has identical branches:\n\n```\nif (isGrouped) {\n  header.column.toggleGrouping();\n} else {\n  header.column.toggleGrouping();\n}\n```\n\nBoth branches do the same thing. This is either dead code (simplify to just `header.column.toggleGrouping()`) or the else-branch was meant to do something different (e.g., start grouping on a non-grouped column vs removing grouping).