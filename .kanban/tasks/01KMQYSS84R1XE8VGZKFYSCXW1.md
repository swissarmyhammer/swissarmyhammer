---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff980
title: 'BadgeListDisplay: first pill nav.left should return to parent field'
---
**Severity**: Low / Design

**File**: `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` (lines 85-87, 91-94)

**What**: The first pill in a badge list has a `nav.right` predicate (claim when parent is focused) but no `nav.left` predicate. The last pill has `nav.left` but no `nav.right`. This means once the user navigates into the pill list, `nav.left` from the first pill is clamped -- there is no keyboard-only way to return focus to the parent field row.

**Suggested fix**: Add a `nav.left` predicate to the first pill that returns focus to `parentMoniker`, symmetrical to how `nav.right` enters from the parent. Similarly, consider what `nav.right` from the last pill should do (advance to the next field row?). #review-finding