---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffc80
title: 'badge-list-nav tests: no coverage for reference-field pill navigation'
---
**Severity**: Low / Test Gap

**File**: `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx`

**What**: All five integration tests use a computed tag field (`commit_display_names: true`). The moniker computation path for reference fields (e.g. `depends_on` where values are entity IDs, not slugs) is different -- it skips the tag-entity-find branch and uses `buildMoniker(targetEntityType, val)` directly. This path has no navigation integration test.

**Suggested fix**: Add a test case with a reference field definition (e.g. `type: { entity: "task" }` without `commit_display_names`) and entity ID values to verify pill monikers and nav predicates work for that code path. #review-finding