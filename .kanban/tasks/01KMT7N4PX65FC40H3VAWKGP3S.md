---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffac80
title: Add tests for UIState::touch_recent and recent_boards
---
ui_state.rs:512-551\n\nMRU recent boards list with no test coverage:\n- `touch_recent(path, name)` — upserts to front, truncates to 20\n- `recent_boards()` — returns the list\n\nTest cases:\n1. touch_recent adds an entry, recent_boards returns it\n2. Touching same path again moves it to front (deduplicates)\n3. List is capped at MAX_RECENT_BOARDS (20)\n4. last_opened field is populated (non-empty string)