---
position_column: done
position_ordinal: ffff8e80
title: Add debounce to search_mentions in multi-select editor
---
**W3: No debounce on `search_mentions` Tauri invoke**

`cm-mention-autocomplete.ts` calls `invoke("search_mentions")` on every keystroke without debouncing. Fast typing generates redundant backend calls.

**Fix:** Add a ~150ms debounce or use CM6's built-in `activateOnTypingDelay` to throttle autocomplete invocations.