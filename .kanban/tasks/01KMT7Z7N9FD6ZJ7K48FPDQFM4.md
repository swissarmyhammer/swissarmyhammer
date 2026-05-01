---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8a80
title: Add tests for DragCompleteCmd::execute (cross-board path)
---
drag_commands.rs:343-362\n\nThe cross-board branch of DragCompleteCmd::execute constructs a JSON result with `cross_board: true` for the Tauri layer. The underlying `transfer_task` is well-tested in cross_board.rs, but the command-layer orchestration that detects a cross-board drag session and delegates to transfer_task is not exercised.\n\nTest should:\n- Set up two boards\n- Start a drag with a target_board_path pointing to a different board\n- Complete the drag\n- Verify the cross_board result JSON structure #coverage-gap