---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffff680
title: isSource detection uses board path comparison, breaks for same-board multi-window
---
**kanban-app/ui/src/lib/drag-session-context.tsx:77**\n\n`isSource` is determined by `event.payload.source_board_path === boardPathRef.current`. When two windows show the *same* board (a supported scenario per the plan's verification step 1), both windows will see `isSource = true` and neither will show the drop overlay.\n\nThe plan explicitly calls for: \"Open two windows showing the same board → drag card from one to the other → card moves between columns.\" This won't work because the target window also thinks it's the source.\n\n**Suggestion:** Include the Tauri window label in the drag session payload (available via `getCurrentWindow().label`). Set `isSource` by comparing window labels, not board paths.