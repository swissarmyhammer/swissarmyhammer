---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9480
title: 'date-editor: onChange in useEffect may fire on mount with the initial value'
---
**File:** `kanban-app/ui/src/components/fields/editors/date-editor.tsx` lines 48-52\n**Severity:** warning\n\nThe `useEffect` that parses the draft and calls `onChange?.(parsed)` runs on mount with the initial value. If the field already has a valid date, this immediately triggers a debounced save of the existing value -- a no-op write to the backend on every editor open. Other editors (number, color, select) only call `onChange` in response to user interaction, which is the correct pattern.\n\nFix: Guard with a `hasMounted` ref or skip when `parsed === initial`. #review-finding