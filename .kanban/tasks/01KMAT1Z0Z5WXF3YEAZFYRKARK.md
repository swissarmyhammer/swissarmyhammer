---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
title: 'entity-icon.tsx: hardcoded entity type to icon mapping'
---
**File:** `kanban-app/ui/src/components/entity-icon.tsx:11-17`\n\nHardcodes `{ task: CheckSquare, tag: Tag, column: Columns, actor: User, board: KanbanSquare }`. Entity definitions should declare their own icon (like field definitions now do), and the component should use the same dynamic lucide lookup. #field-special-case