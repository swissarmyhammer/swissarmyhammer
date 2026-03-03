---
position_column: todo
position_ordinal: b9
title: Fix exit2_tests::post_tool_use_exit2_feeds_context
---
Test in agent-client-protocol-extras/tests/e2e_hooks/exit2_tests.rs:97 fails with: 'PostToolUse exit-2 should feed stderr as context'. The hook executor is not feeding stderr as context when a hook exits with code 2.