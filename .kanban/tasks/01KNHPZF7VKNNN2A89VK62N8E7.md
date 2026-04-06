---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8780
title: 'Coverage: WorkspaceWatcher::start event handling (watcher.rs, ~16 lines)'
---
## What

`swissarmyhammer-treesitter/src/watcher.rs::WorkspaceWatcher::start` has ~16 uncovered lines in the notify event handler for Create/Modify/Remove OS events.

## Acceptance Criteria

- [ ] Test starts a WorkspaceWatcher on a temp directory
- [ ] Test writes a new file and verifies a FileEvent::Created variant is received
- [ ] Test modifies the file and verifies a FileEvent::Modified variant is received
- [ ] Test deletes the file and verifies a FileEvent::Removed variant is received

## Tests

- [ ] Add test in `swissarmyhammer-treesitter/src/watcher.rs` (or `tests/`) that starts a watcher, performs file operations, and asserts the correct FileEvent variants are received on the channel
- [ ] `cargo nextest run -p swissarmyhammer-treesitter` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap