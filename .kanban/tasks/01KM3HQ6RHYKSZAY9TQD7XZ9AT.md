---
assignees:
- claude-code
position_column: done
position_ordinal: fffffff580
title: set_active_view/set_inspector_stack silently no-op when window entry missing
---
**commands.rs:232-248**

`set_active_view` and `set_inspector_stack` both do `if let Some(ws) = config.windows.get_mut(label)` — if the window label doesn't have an entry yet (e.g. main window on first launch before geometry save), the mutation is silently dropped. The caller gets back a success response but the state was never persisted.

**Suggestion:** Create the entry on demand (upsert pattern) like the `on_window_event` handler does for the main window, or return an error so the frontend knows it wasn't saved.

- [ ] Ensure set_active_view creates a WindowState entry if one doesn't exist
- [ ] Same for set_inspector_stack
- [ ] Verify with test: call set_active_view for a label that has no entry yet