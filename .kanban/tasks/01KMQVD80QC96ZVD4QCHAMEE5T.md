---
assignees:
- claude-code
position_column: todo
position_ordinal: cd80
title: '[Correctness/Low] broadcastRef pattern is correct -- no stale closure issue'
---
**File:** `kanban-app/ui/src/components/app-shell.tsx:153-155`\n\n**What:** The nav command handlers use `broadcastRef.current(\"nav.up\")` rather than calling `broadcastNavCommand` directly. This is correct: `broadcastNavCommand` is destructured from `useEntityFocus()` and while it's a stable `useCallback`, the ref pattern adds defense-in-depth. The ref is updated on every render (`broadcastRef.current = broadcastNavCommand`) so execute closures inside the `useMemo` always see the latest function.\n\n**Verdict:** Correct pattern. No issue." #review-finding