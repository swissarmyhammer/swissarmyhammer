---
position_column: todo
position_ordinal: b1
title: 'Fix: e2e_hooks json_continue_tests::post_tool_use_continue_false_cancels'
---
PostToolUse continue:false should send Cancel to cancel channel but timed out after 5s. File: agent-client-protocol-extras/tests/e2e_hooks/json_continue_tests.rs:96 #test-failure