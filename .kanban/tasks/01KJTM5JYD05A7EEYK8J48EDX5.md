---
position_column: todo
position_ordinal: c3
title: Fix json_specific_output_tests::post_tool_use_additional_context
---
Test in agent-client-protocol-extras/tests/e2e_hooks/json_specific_output_tests.rs:95 fails with: 'PostToolUse additionalContext should deliver via context channel'. The hook executor is not delivering additionalContext from JSON output to the context channel.