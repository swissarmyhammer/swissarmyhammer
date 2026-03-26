---
position_column: done
position_ordinal: ffaf80
title: '[nit] get_uncommitted_changes called twice in execute path'
---
**Severity: nit**\n**File:** swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs:356-424\n\nIn the `execute` method, `get_uncommitted_changes()` is called once to check if the tree is clean (line 360), and then unconditionally called again at line 418 to merge uncommitted changes into the final result. On the clean-main-defaulting-to-HEAD~1 path, this means the same function runs twice when the first call already returned empty. Consider caching the first result and reusing it." #review-finding