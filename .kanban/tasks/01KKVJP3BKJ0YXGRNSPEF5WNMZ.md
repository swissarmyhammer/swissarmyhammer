---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb380
title: 'WARNING: Arc::get_mut race window in open_board — watcher may never start'
---
File: kanban-app/src/state.rs lines 362-369 — After inserting the new Arc into `self.boards`, a second write-lock acquisition follows to call `Arc::get_mut`. Between the two lock acquisitions any concurrent reader (e.g. an incoming Tauri command from a quickly-responding frontend) could clone the Arc, causing `Arc::get_mut` to return `None` and the file watcher to silently not start.\n\nThis is a TOCTOU gap: insert → drop write lock → reacquire write lock → get_mut. If another task holds a clone in the interim the watcher is permanently silenced with no error logged.\n\nSuggestion: start the watcher before inserting into the map, or keep the Arc in a local variable, start the watcher on it, then insert it. Alternatively use a Mutex-wrapped Option inside BoardHandle to set the watcher after insertion.\n\nVerification step: check whether any concurrent Tauri command can clone the board Arc between the two write-lock sections in open_board, and confirm a None result from Arc::get_mut is observable." #review-finding