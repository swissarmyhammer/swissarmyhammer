---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffe480
title: loadBoardEntity switches active board as side effect
---
quick-capture.tsx:60-79\n\n`loadBoardEntity` calls `set_active_board` to switch to the selected board, fetches data, then switches back. This is a race condition: if the user interacts with the main app window while quick capture is loading, the active board flip-flops. It also fires on every `selectedPath` change.\n\nSuggestion: Use `get_entity` or a board-specific query that doesn't require switching the active board. Or pass the board path to `get_board_data` if the backend supports it.