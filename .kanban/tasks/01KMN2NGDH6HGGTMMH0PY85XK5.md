---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff280
title: 'Fix entity-commands.test.ts failures (4 tests): useEntityCommands returns empty array / missing entity.inspect'
---
useEntityCommands hook returns 0 commands instead of the expected schema-derived commands including entity.inspect and entity.archive.\n\nFailing tests:\n- returns CommandDefs with resolved names from schema (length 0, expected > 0)\n- resolves template name using entity field values (entity.archive not found)\n- appends extraCommands after schema commands (first command is 'task.untag' not 'entity.inspect')\n- entity.inspect execute calls the inspect function (entity.inspect not found)\n\nFile: `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/lib/entity-commands.test.ts`"