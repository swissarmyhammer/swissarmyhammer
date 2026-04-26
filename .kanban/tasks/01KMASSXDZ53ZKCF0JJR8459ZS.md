---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8a80
title: 'multi-select-editor.tsx: hardcoded actor entity type check for AvatarDisplay'
---
**File:** `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx:235`\n\n```ts\ntargetEntityType === \"actor\"\n```\n\nRenders AvatarDisplay specifically for actors, colored pills for everything else. The display component used for multi-select items should be driven by the target entity's schema, not a hardcoded entity type check. #field-special-case