---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8580
title: No test for StoreContext with multiple stores dispatching undo to correct store
---
**swissarmyhammer-store/src/context.rs:58-86**\n\nThe existing `register_and_undo_dispatches_correctly` test only registers one store. The `undo()` method iterates all stores calling `has_entry()` on each until it finds the owner. There is no test verifying:\n1. With 2+ stores, undo dispatches to the correct one (not the first one).\n2. `NoProvider` error is returned when no store owns the entry.\n\n**Severity: warning**\n\n**Suggestion:** Add tests with two stores where the undo target belongs to the second store, and a test where the entry belongs to neither.\n\n**Subtasks:**\n- [ ] Add test: undo dispatches to second store when first store does not own the entry\n- [ ] Add test: undo returns NoProvider when no store owns the entry\n- [ ] Same tests for redo\n- [ ] Verify all tests pass" #review-finding