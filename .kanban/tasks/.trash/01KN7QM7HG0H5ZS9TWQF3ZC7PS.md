---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: Fix 31 test files failing due to unresolved @/ imports (missing modules)
---
31 test files fail to load because they import modules that do not exist on disk. The missing modules are: @/lib/ui-state-context, @/lib/schema-context, @/lib/drop-zones, @/lib/field-update-context, @/lib/entity-focus-context, @/lib/command-scope, @/components/fields/displays/markdown-display, @/components/fields/registrations, @/components/focus-scope, @/lib/log, @/lib/moniker, @/types/kanban, @/lib/file-drop-context, @/components/ui/tooltip, @/components/mention-pill. These are imported from both test files and source files. This is a systemic issue -- either these modules were deleted/moved or the vitest project config path aliases are misconfigured. #test-failure