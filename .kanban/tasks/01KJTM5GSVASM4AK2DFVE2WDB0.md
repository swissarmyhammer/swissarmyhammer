---
position_column: todo
position_ordinal: c2
title: Fix json_continue_tests::post_tool_use_failure_continue_false_cancels
---
Test in agent-client-protocol-extras/tests/e2e_hooks/json_continue_tests.rs:122 fails with: 'PostToolUseFailure continue:false should send Cancel to cancel channel'. Same root cause as the post_tool_use variant.