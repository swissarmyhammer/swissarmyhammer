---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: broadcastNavCommand O(N) linear scan on every keystroke
---
**Severity: Medium (Performance)**

In `kanban-app/ui/src/lib/entity-focus-context.tsx`, `broadcastNavCommand` iterates over ALL registered claim predicates (every FocusScope on the page) for every navigation keystroke. For a board with 100 tasks across 3 columns, that is 100+ task scopes x ~5 predicates each = 500+ predicate evaluations per keypress.

Each predicate is a simple string comparison (`f === someMoniker`), so the constant factor is small. But this scales linearly with board size. The current design is "first match wins" with Map insertion order, which is correct.

**Recommendation:** This is acceptable for MVP. If profiling shows keyboard lag on large boards (200+ tasks), consider indexing predicates by `(command, focusedMoniker)` pairs in a Map for O(1) lookup instead of linear scan. No action needed now, but document the scaling characteristic. #review-finding