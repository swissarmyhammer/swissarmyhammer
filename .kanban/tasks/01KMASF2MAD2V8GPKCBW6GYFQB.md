---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
title: 'tag-inspector.tsx: entirely hardcoded field layout'
---
**File:** `kanban-app/ui/src/components/tag-inspector.tsx`\n\nThe entire tag inspector is a hardcoded component with field names baked in (`tag_name`, `color`, `description`). It should not exist — the generic `EntityInspector` should handle tag entities the same as any other entity type, driven by the tag entity's field definitions. #field-special-case