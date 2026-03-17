---
assignees:
- claude-code
depends_on:
- 01KKWF8YVWQ4XNC24DV8YCK650
position_column: done
position_ordinal: ffffffce80
title: Frontend drag session provider and hooks
---
## What
Create a React context that manages drag session state across windows, listening to Tauri events and providing hooks for board-view integration.

**Files:**
- `kanban-app/ui/src/lib/drag-session-context.tsx` (new) — DragSessionProvider, useDragSession hook
- `kanban-app/ui/src/App.tsx` — wire DragSessionProvider into provider tree

**DragSessionState type:**
```typescript
interface DragSessionState {
  active: boolean;
  entityType: string;
  entityId: string;
  sourceBoardPath: string;
  sourceWindowLabel: string;
  entitySnapshot: Record<string, unknown>;
}
```

**DragSessionProvider:**
- Listens for `drag-session-started` → sets state, compares window label via `getCurrentWindow().label` to determine if THIS window is the source
- Listens for `drag-session-cancelled` → clears state
- Listens for `drag-session-completed` → clears state
- Exposes: `startDragSession(entityType, entityId)`, `cancelDragSession()`, `completeDragSession(targetColumn, beforeId?, afterId?, copy?)`
- Exposes: `isSourceWindow` (boolean), `isRemoteDragActive` (active && !isSourceWindow), `sessionState`

**useDragSession() hook:**
- Returns session state + actions from context
- Used by board-view.tsx (source integration) and cross-window-drop-overlay (target integration)

**Provider wiring in App.tsx:**
- Add `<DragSessionProvider>` wrapping main content, inside ActiveBoardPathProvider so it can read the board path

## Acceptance Criteria
- [ ] DragSessionProvider listens for all three drag-session Tauri events
- [ ] isSourceWindow correctly identifies the drag origin window
- [ ] isRemoteDragActive is true only in non-source windows during active session
- [ ] startDragSession invokes the backend command
- [ ] completeDragSession invokes the backend command with target board path from context
- [ ] Provider wired into App.tsx

## Tests
- [ ] Manual: start drag in window A, verify drag-session-started event received in window B
- [ ] Manual: cancel drag, verify both windows clear state
- [ ] `npm run build` compiles without errors