---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffa80
title: 'NIT: EntityRow, RowSelector, GridCellScope use anonymous inline prop types'
---
**File**: kanban-app/ui/src/components/data-table.tsx\n\n**What**: `EntityRow` and `RowSelector` use anonymous inline object types for their props (e.g. `{ entityMk: string; isCursorRow: boolean; ... }`). `GridCellScope` has a named interface but `EntityRow` and `RowSelector` do not.\n\n**Suggestion**: Extract named interfaces `EntityRowProps` and `RowSelectorProps`.\n\n**Subtasks**:\n- [ ] Add named EntityRowProps and RowSelectorProps interfaces\n- [ ] Verify fix by running tests #review-finding