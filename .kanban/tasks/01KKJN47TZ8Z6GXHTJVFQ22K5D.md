---
position_column: done
position_ordinal: z00
title: '[blocker] `start_indexing_workers` and `start_indexing_workers_after_promotion` are near-identical — duplicated code'
---
**File:** `swissarmyhammer-tools/src/mcp/server.rs`\n**Severity:** blocker\n\n`start_indexing_workers` (lines ~495–523) and `start_indexing_workers_after_promotion` (lines ~527–624) share identical tree-sitter and file-watcher spawn logic. The only difference is that the promotion variant adds a third task that polls for the LSP supervisor. This violates DRY in a meaningful way: any bug fix or new worker added to one copy must be manually mirrored to the other.\n\n**Fix:** Extract a shared `spawn_ts_and_watcher_workers(workspace_root, shared_db)` helper called by both, then add the LSP polling task only in the promotion path." #review-finding