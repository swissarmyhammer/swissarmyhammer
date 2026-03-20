---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffac80
title: entity events re-fetch without board_path — second window fetches wrong board's entity
---
kanban-app/ui/src/App.tsx:206-216, 249-254\n\nIn the `entity-created` and `entity-field-changed` handlers, the code calls `invoke(\"get_entity\", { entityType, id })` without passing `boardPath`. In a multi-window scenario, window B (showing board B) will receive entity events from window A's mutations (because Tauri emits to all windows). Window B then fetches `get_entity` for that entity ID against the global active board — which may be board A at that moment — and incorrectly patches its own entity store with entities from another board.\n\nSuggestion: Pass `boardPath: activeBoardPathRef.current` to `get_entity` invocations inside the event handlers. The same pattern used in `refreshBoards` should be applied here." #review-finding