---
position_column: todo
position_ordinal: b2
title: 'Fix: e2e_hooks json_continue_tests::post_tool_use_failure_continue_false_cancels'
---
PostToolUseFailure continue:false should send Cancel but timed out after 5s. File: agent-client-protocol-extras/tests/e2e_hooks/json_continue_tests.rs #test-failure