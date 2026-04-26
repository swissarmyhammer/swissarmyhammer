---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffff980
title: CommandPalette SearchResultItem onClick calls inspectEntity() directly instead of dispatching entity.inspect command
---
**File:** `kanban-app/ui/src/components/command-palette.tsx`, lines 512-517\n\n**Anti-pattern:** When a user clicks a search result, the handler calls `inspectEntity(entityMoniker)` directly. This is almost identical to the EntityCard Info button issue. The SearchResultItem is already wrapped in a FocusScope with `useEntityCommands`, so the `entity.inspect` command is registered -- but the click handler ignores the scope chain.\n\n**Current code:**\n```tsx\nonClick={() => {\n  if (inspectEntity) {\n    onClose();\n    inspectEntity(entityMoniker);\n  }\n}}\n```\n\n**Also affected:** `executeSelectedResult` (line 200-217) does the same direct call when Enter is pressed on a search result.\n\n**Correct pattern:** Should dispatch the `entity.inspect` command through the scope chain for the selected result's FocusScope.\n\n**Severity:** Warning. The FocusScope is set up correctly for context menus and double-click (which both work through the command system), but the primary click and Enter actions bypass it.\n\n**Scope chain impact:** Search result selection is not logged as a command dispatch. #review-finding