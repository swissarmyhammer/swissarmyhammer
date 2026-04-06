---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc180
title: 'NIT: MultiSelectEditor colorMap hardcodes ''color'' field name'
---
**File**: kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx (colorMap useMemo)\n\n**What**: The colorMap accesses `getStr(e, 'color', '888888')` using a hardcoded field name `'color'`. The comment says 'color' is a \"universal entity property\" which is a convention, not a schema-declared property.\n\n**Suggestion**: If this truly is universal, consider adding a `color_field` property to the mention config in the schema, similar to how `displayField` is already schema-driven.\n\n**Subtasks**:\n- [ ] Evaluate whether color_field should be added to mention config\n- [ ] Verify fix by running tests #review-finding