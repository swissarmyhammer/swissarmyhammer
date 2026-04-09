---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffe780
title: classify_event uses tracing::info for per-notification filesystem events
---
kanban-app/src/watcher.rs: classify_event function\n\nThe newly added `tracing::info!` call inside `classify_event` fires for every filesystem notification event the watcher receives. On active boards with many files, this produces high-volume log output at info level. The existing `tracing::trace!` for the skip path is correct, but the matching success path should be `debug` or `trace`, not `info`.\n\nSuggestion: Change the `tracing::info!` to `tracing::debug!` to keep it useful for troubleshooting without flooding production logs.\n\nVerification: Run with RUST_LOG=info and edit several files — confirm classify_event lines no longer appear. #review-finding