---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffee80
title: 'Test file_watcher: watching lifecycle, retry logic, and MCP notifications'
---
File: swissarmyhammer-tools/src/mcp/file_watcher.rs (17.1%, 116 uncovered lines)

Uncovered functions:
- FileWatcher::start_watching() - the main watch loop with debounce
- stop_watching() - cleanup
- FileWatcherCallback (MCP notification bridge): on_file_changed(), on_error()
- retry_with_backoff() - generic retry utility
- FileWatchingManager: start_file_watching(), stop_file_watching(), is_retryable_fs_error()

Also error_handling.rs (0%, 41 uncovered lines):
- ErrorHandler: reload_prompts(), reload_prompts_with_retry(), reload_prompts_internal(), is_retryable_fs_error()

These are async/reactive components that need careful test setup with mock filesystems or tempdir watchers. #coverage-gap