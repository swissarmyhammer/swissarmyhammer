---
position_column: done
position_ordinal: c1
title: 'Fix reload_prompts tests: detects_content_changes, detects_deleted_prompts, detects_new_prompts'
---
3 tests in swissarmyhammer-tools/src/mcp/tests.rs fail with assertion errors. They test prompt reload change detection but assert!() fails indicating reload does not detect changes after content modification, deletion, or addition of prompts. Files: /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-tools/src/mcp/tests.rs (lines 486, 538, 592)