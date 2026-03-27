---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8f80
title: 'command-palette.tsx: hardcoded "board" entity type for switch handler'
---
**File:** `kanban-app/ui/src/components/command-palette.tsx:206`\n\n```ts\nif (result.entity_type === \"board\" && onSwitchBoard) {\n```\n\nHardcodes that \"board\" results get special handling (calls `onSwitchBoard` instead of normal inspect). Board-specific behavior should be driven by entity commands, not an entity type check. #field-special-case