---
title: 'Fix test_tag_nonexistent_tag: assertion expects error but operation succeeds'
position:
  column: done
  ordinal: b5
---
Test `mcp::tools::kanban::tests::test_tag_nonexistent_tag` in swissarmyhammer-tools fails. The test at line 2500 of `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` asserts `result.is_err()` when tagging a task with a nonexistent tag ID, but the operation returns Ok instead of Err. Either the kernel's `tag task` operation should return an error for nonexistent tags, or the test expectation is wrong.