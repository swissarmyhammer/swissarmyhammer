---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb580
title: Changelog uses synchronous I/O from async context
---
changelog.rs:143-156\n\nThe `PerspectiveChangelog::append` method uses `std::fs::OpenOptions` (synchronous blocking I/O) but is called from within async `Execute` implementations (add.rs:103, update.rs:132, delete.rs:47). These async methods hold an `RwLock` write guard when calling the changelog. Blocking I/O while holding an async lock can stall the tokio runtime if the file system is slow.\n\nThe `PerspectiveContext` correctly uses `tokio::fs` for its I/O. The changelog should match.\n\nThis is low severity because:\n- JSONL appends are typically fast (single line, sequential write)\n- The changelog write is already fire-and-forget (errors are logged, not propagated)\n- The board is typically single-user\n\nSuggestion: Either switch to `tokio::fs` for consistency, or use `tokio::task::spawn_blocking` to move the write off the async executor. Alternatively, document the intentional choice.\n\nVerification: Code review of the updated implementation." #review-finding