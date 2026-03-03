---
position_column: todo
position_ordinal: c4
title: Fix json_specific_output_tests::post_tool_use_failure_additional_context
---
Test in agent-client-protocol-extras/tests/e2e_hooks/json_specific_output_tests.rs:125 fails with: 'PostToolUseFailure additionalContext should deliver via context channel'. Same root cause as post_tool_use variant.