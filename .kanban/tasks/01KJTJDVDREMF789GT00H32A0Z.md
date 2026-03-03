---
position_column: done
position_ordinal: h6
title: 'Fix test: attachment::add::tests::test_add_attachment'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/add.rs:282` fails with assertion `left == right` (left: 0, right: 1). After adding an attachment, the task's attachment count is 0 instead of the expected 1.

**Resolution:** Test passes as-is. All 210 kanban package tests pass. No code changes required -- the underlying issue was already fixed in a prior commit.