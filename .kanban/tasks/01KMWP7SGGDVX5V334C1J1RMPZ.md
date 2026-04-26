---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffff280
title: Add tests for register_merge_drivers error path (init.rs)
---
swissarmyhammer-kanban/src/board/init.rs:68-135 (in worktree agent-a361f6ed)\n\n`async fn register_merge_drivers(board_root: &Path) -> Result<(), io::Error>`\n\nHappy paths well covered (7 tests). Missing:\n- I/O error writing `.git/config` (e.g., read-only file) — should propagate as KanbanError::Io\n- I/O error writing `.gitattributes`\n- Board root with no parent (line 72 returns Ok(()))\n- Pre-existing `.gitattributes` with other content — verify merge drivers are appended without disturbing existing entries #coverage-gap