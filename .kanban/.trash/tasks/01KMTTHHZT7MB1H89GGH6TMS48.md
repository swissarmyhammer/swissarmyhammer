---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: '[BLOCKER] GitMergeDrivers placed in wrong integration point (sah init vs kanban init board)'
---
`swissarmyhammer-cli/src/commands/install/components/mod.rs`\n\nThe `GitMergeDrivers` init component was wired into `sah init` (the global CLI install command) instead of the kanban board's `InitBoard::execute()` in `swissarmyhammer-kanban/src/board/init.rs`. Merge drivers are a per-board concern: they configure `.gitattributes` and `.git/config` for the `.kanban/` files that live alongside a specific board. Installing them globally via `sah init` means they fire for every repo that runs `sah init`, not just repos that actually have a kanban board, and they won't be set up automatically when someone creates a new board with `init board`.\n\nMove the `GitMergeDrivers` logic out of `install/components/mod.rs` and invoke it from `InitBoard::execute()` in the kanban crate instead. The `.gitattributes` and `.git/config` entries should be scoped to the board directory being initialized." #review-finding