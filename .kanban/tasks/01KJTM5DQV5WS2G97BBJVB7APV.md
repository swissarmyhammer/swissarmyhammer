---
position_column: todo
position_ordinal: c0
title: Fix exit2_tests::post_tool_use_failure_exit2_feeds_context
---
Test in agent-client-protocol-extras/tests/e2e_hooks/exit2_tests.rs:135 fails with: 'PostToolUseFailure exit-2 should feed stderr as context'. Same root cause as post_tool_use variant -- exit-2 stderr not being delivered as context.