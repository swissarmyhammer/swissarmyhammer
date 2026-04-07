---
assignees:
- claude-code
position_column: todo
position_ordinal: a580
title: 'Fix failing test: context::tests::test_fields_accessor'
---
Test at `swissarmyhammer-kanban/src/context.rs:834` fails with assertion `left == right` (left: 12, right: 11). The test expects 11 fields but 12 are returned, suggesting a new field was added without updating this test. #test-failure