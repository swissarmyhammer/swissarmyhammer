---
position_column: done
position_ordinal: ffc680
title: 'ColumnReorderCmd: make reorder atomic (rollback on partial failure)'
---
**File:** `swissarmyhammer-kanban/src/commands/column_commands.rs`\n\n**What:** ColumnReorderCmd iterates columns calling `run_op` for each. If one fails mid-loop, earlier columns are already reordered — leaving the board in a half-updated state.\n\n**Fix:** Collect all operations and only commit if all succeed, or accept the current behavior with a comment explaining why partial failure is acceptable (columns are independent).\n\n- [ ] Evaluate whether partial failure is actually harmful for column reorder\n- [ ] Either add rollback/batch semantics or document the acceptable risk\n- [ ] Verify tests pass