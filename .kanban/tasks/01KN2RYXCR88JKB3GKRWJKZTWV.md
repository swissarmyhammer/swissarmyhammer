---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9880
title: 'multi-select-editor: handleChange inserted mid-comment, splitting it into two orphan fragments'
---
**File:** `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` lines 276-298\n**Severity:** warning\n\nThe `handleChange` callback was inserted between line 279 (ending with `// Only wait if the schema is still loading -- some entity types (e.g.`) and line 298 (`// attachment) don't have a mention_prefix and that's fine.`). This splits a previously coherent multi-line comment into two disconnected fragments that make no sense individually.\n\nFix: Move `handleChange` above the comment block, or reconstruct the comment around the new code. #review-finding