---
position_column: todo
position_ordinal: c1
title: Fix json_continue_tests::post_tool_use_continue_false_cancels
---
Test in agent-client-protocol-extras/tests/e2e_hooks/json_continue_tests.rs:96 fails with: 'PostToolUse continue:false should send Cancel to cancel channel'. The hook executor is not sending Cancel when a JSON hook returns continue:false.