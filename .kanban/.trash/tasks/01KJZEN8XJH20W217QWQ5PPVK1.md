---
position_column: done
position_ordinal: k6
title: Fix test_reload_prompts_detects_content_changes
---
File: swissarmyhammer-tools/src/mcp/tests.rs:488. Assertion failure: "Reload after content change should detect changes". The prompt reload test does not detect content changes correctly. #test-failure