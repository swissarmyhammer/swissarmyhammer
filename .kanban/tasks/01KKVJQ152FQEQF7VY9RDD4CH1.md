---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff8180
title: 'WARNING: dscl subprocess in macos_profile_picture blocks the async executor'
---
File: kanban-app/src/state.rs lines 683-709 — `macos_profile_picture` calls `std::process::Command::new(\"dscl\").output()`, which is a synchronous blocking call. It is invoked from `ensure_os_actor` which is called from the async `open_board` path. Running a blocking subprocess on the async executor will stall the Tokio thread for the duration of the dscl call (typically 50-500ms on macOS).\n\nSuggestion: wrap the dscl call with `tokio::task::spawn_blocking` so it runs on the blocking thread pool, or defer it to a background task so the board open path is not delayed.\n\nVerification step: search for any `spawn_blocking` wrapping around `ensure_os_actor` or `macos_profile_picture`; confirm none exists." #review-finding