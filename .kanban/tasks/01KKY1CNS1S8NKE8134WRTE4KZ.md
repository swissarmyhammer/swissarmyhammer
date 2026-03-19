---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffe280
title: Add tests for board-switching, event scoping, and window restoration logic
---
No test coverage for:\n- board-changed handler: closed board fallback, still-open refresh\n- board-opened handler: focused window switches, unfocused ignores\n- Window restoration: main window recreates secondary windows for open boards\n- open_board emitting board-opened event\n- cross-window overlay visibility (source vs target window)