---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa480
title: Test treesitter FileWatcher start/event handling
---
File: swissarmyhammer-treesitter/src/watcher.rs (65.1% coverage, 41/63 lines)\n\nUncovered code:\n- start() method - the async file watching setup and event loop (lines 119-160)\n  - Creates AsyncDebouncer, starts watching recursively\n  - Spawns event handler task that categorizes events into changed/removed\n  - Calls callback.on_files_changed() and callback.on_files_removed()\n\nThis is the core file watching functionality. Testing requires creating temp files, modifying them, and verifying callbacks fire. The debounce logic (500ms) needs careful async test timing." #coverage-gap