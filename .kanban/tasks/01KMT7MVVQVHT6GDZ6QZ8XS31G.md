---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffb380
title: Add tests for UIState context menu methods
---
ui_state.rs:411-424\n\nTwo context menu methods with no test coverage:\n- `set_context_menu_ids(ids: HashSet<String>)` — stores the set\n- `is_context_menu_id(id)` — checks membership\n\nTest cases:\n1. Set IDs and verify membership with is_context_menu_id\n2. Non-member returns false\n3. Replacing the set clears previous IDs