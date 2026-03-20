---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9880
title: 'WARNING: sync_open_boards_to_config called while boards write lock may be held'
---
File: kanban-app/src/state.rs — `close_board` holds the boards write lock across a call to `sync_open_boards_to_config`, which then acquires a boards *read* lock internally. Tokio's `RwLock` is not reentrant; this will deadlock on the current thread.\n\nIn `close_board` (lines 533–544), the block ends with the write lock dropped before calling `sync_open_boards_to_config`, so the deadlock does not actually fire today. However, the doc comment on `sync_open_boards_to_config` (line 562) says \"must NOT be called while the boards write lock is held\", meaning the hazard is latent and only one refactor away from becoming a real deadlock.\n\nSuggestion: either restructure `close_board` to collect the updated paths inside the write-lock block and skip the helper, or make the doc comment more prominent with a `debug_assert!` that the lock is not currently acquired.\n\nVerification step: read `close_board` carefully; confirm the boards write lock is dropped before the helper is called, and add a comment that proves this invariant at the call site." #review-finding