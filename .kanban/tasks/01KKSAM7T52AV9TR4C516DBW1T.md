---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffea80
title: Entity event listener recreated on every boardEntity change
---
quick-capture.tsx:108-134\n\nThe `useEffect` that sets up Tauri event listeners has `[boardEntity, loadBoards]` as deps. Since `boardEntity` is state that changes when events fire, this creates a cycle: event fires → setBoardEntity → effect re-runs → unlistens old + re-listens. This causes brief gaps where events are missed.\n\nSuggestion: Use a ref for boardEntity inside the listener instead of closing over the state value. Remove `boardEntity` from the deps array.