---
title: 'Fix test_kanban_schema_has_all_operations: op enum count mismatch (expected 50, got 46)'
position:
  column: done
  ordinal: a1
---
The test `integration::mcp_tools_registration::test_kanban_schema_has_all_operations` in `swissarmyhammer-cli` fails because the kanban tool schema registers 46 operations in the op enum, but the test expects 50. The test is at `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs:222`. Either new operations were added to the test expectation without implementing them, or operations were removed without updating the test. The assertion needs to match the actual operation count.