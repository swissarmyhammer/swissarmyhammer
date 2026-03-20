---
position_column: done
position_ordinal: ffffd480
title: Fix 3 failing prompt reload tests in swissarmyhammer-tools (test_reload_prompts_detects_*)
---
Tests in swissarmyhammer-tools/src/mcp/tests.rs lines 486, 538, 592 fail with assertions about detecting prompt changes. Affected tests: test_reload_prompts_detects_content_changes, test_reload_prompts_detects_new_prompts, test_reload_prompts_detects_deleted_prompts