---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffd380
title: Duplicated BoardFocusBridge / GridFocusBridge should be a shared component
---
**Severity: Low** (Code quality / DRY violation)

**Files:**
- `kanban-app/ui/src/components/board-view.tsx`, lines 57-73 (BoardFocusBridge)
- `kanban-app/ui/src/components/grid-view.tsx`, lines 37-53 (GridFocusBridge)

**Problem:** BoardFocusBridge and GridFocusBridge are character-for-character identical. Both read CommandScopeContext, register/unregister a scope, and setFocus on moniker change. This is a copy-paste that will diverge over time.

**Recommendation:** Extract a single `CursorFocusBridge` (or similar) component into a shared module (e.g. `@/components/cursor-focus-bridge.tsx`). Both board-view and grid-view import and use it. #review-finding