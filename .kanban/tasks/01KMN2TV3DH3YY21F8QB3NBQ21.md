---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffc080
title: 'Fix failing tests: useEntityCommands - CommandDefs, template names, extraCommands, entity.inspect (4 tests)'
---
File: /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/lib/entity-commands.test.ts\n\nFailing tests:\n- useEntityCommands > returns CommandDefs with resolved names from schema\n- useEntityCommands > resolves template name using entity field values\n- useEntityCommands > appends extraCommands after schema commands\n- useEntityCommands > entity.inspect execute calls the inspect function #test-failure