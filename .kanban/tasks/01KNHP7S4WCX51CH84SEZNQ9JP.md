---
assignees:
- claude-code
depends_on:
- 01KNHP391SXAQ5H2YXEK2MYJD1
position_column: done
position_ordinal: fffffffffffffffffffff680
title: 'NIT: board-view.tsx constructs task/column monikers instead of using entity.moniker'
---
**File:** `kanban-app/ui/src/components/board-view.tsx` — multiple sites\n\n**What:** Several moniker construction sites:\n- `moniker(\"board\", \"board\")` for boardMoniker\n- `moniker(\"task\", id)` in columnTaskMonikers map\n- `moniker(\"column\", col.id)` and `fieldMoniker(\"column\", col.id, \"name\")` in allBoardHeaderMonikers\n- `moniker(\"column\", columns[0].id)` for initial focus\n\nThese are used for focus navigation (claimWhen predicates, FocusScope monikers). The entities are available (`board`, `columns`, `tasks`).\n\n**Why:** For navigation monikers within the board view, these are structural identifiers for the focus system. They work correctly for non-archived entities. However, the board entity moniker could use `board.board.moniker`, and task/column monikers could use `entity.moniker` for consistency with the migration direction.\n\n**Suggestion:** After the `entityFromBag` root fix, migrate to `entity.moniker` where the entity is in scope. For `fieldMoniker` calls (which scope to a field within an entity), these remain correct since `fieldMoniker` is a separate concept — it extends the entity moniker with a field suffix. These should use `entity.moniker` as the base instead of reconstructing from parts.\n\n**Verification:** Keyboard navigation in board view continues to work after migration. #review-finding