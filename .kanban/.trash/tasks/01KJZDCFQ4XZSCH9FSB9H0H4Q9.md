---
position_column: done
position_ordinal: k0
title: 'Fix 4 reload_prompts tests: CWD deleted by concurrent test'
---
Tests in swissarmyhammer-tools/src/mcp/tests.rs panic with "Failed to get current dir: No such file or directory". Affected tests: test_reload_prompts_detects_new_prompts (line 498), test_reload_prompts_detects_no_changes (line 392), test_reload_prompts_detects_deleted_prompts (line 550), test_reload_prompts_detects_content_changes (line 447). Root cause: tests change CWD to a temp dir that gets deleted by another test running concurrently. #test-failure