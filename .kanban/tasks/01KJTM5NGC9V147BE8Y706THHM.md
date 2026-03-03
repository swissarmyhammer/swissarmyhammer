---
position_column: todo
position_ordinal: c5
title: Fix json_specific_output_tests::notification_additional_context
---
Test in agent-client-protocol-extras/tests/e2e_hooks/json_specific_output_tests.rs:175 fails with: 'Notification additionalContext should deliver via context channel'. The hook executor is not delivering additionalContext from Notification JSON output to the context channel.