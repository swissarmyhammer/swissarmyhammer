---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffbc80
title: 'WARNING: GridView resolveGroupLabel hardcodes field.type.kind === ''reference'' check'
---
**File**: kanban-app/ui/src/components/grid-view.tsx (resolveGroupLabel callback)\n\n**What**: `resolveGroupLabel` checks `fieldDef.type.kind !== 'reference'` and then accesses `fieldDef.type.entity as string | undefined`. This dispatches on `field.type.kind` which the JS/TS review guidelines prohibit: \"Components dispatch on configured properties — never on `field.type.kind`.\"\n\n**Why**: The group label resolution strategy should be declared as a field property (e.g., `field.group_label_resolver: 'reference'`) rather than inferred from the kind.\n\n**Suggestion**: Add a `group_label_resolver` or `reference_display` property to FieldDef that the frontend can check without inspecting `type.kind`.\n\n**Subtasks**:\n- [ ] Design a metadata property for group label resolution strategy\n- [ ] Update resolveGroupLabel to use the property instead of kind check\n- [ ] Verify fix by running tests #review-finding