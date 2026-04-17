---
assignees:
- claude-code
position_column: review
position_ordinal: '8280'
title: Extract DataTable into sub-components (data-table.tsx)
---
The DataTable function (~377 lines) exceeds the 50-line code-quality validator limit. Extract into: useDataTableConfig hook, DataTableHeader component, DataTableBody component, DataTableCell component, GroupHeaderRow component. DataTable becomes a ~30-40 line orchestrator. No behavior changes.