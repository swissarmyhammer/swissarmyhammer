---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffc580
title: '[nit] VirtualBadgeListAdapter missing JSDoc on adapter function'
---
File: kanban-app/ui/src/components/fields/registrations/virtual-badge-list.tsx\n\nThe VirtualBadgeListAdapter function has a one-line JSDoc but the module-level comment is more detailed. The adapter function itself could benefit from explaining why a separate adapter is needed (bridging FieldDisplayProps which includes fieldDef/entityType/entityId to the simpler VirtualTagDisplayProps which only needs value).\n\nSuggestion: Expand the function-level comment to clarify the prop narrowing purpose. #review-finding