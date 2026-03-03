---
position_column: done
position_ordinal: f9
title: KanbanLookup task branch still uses deprecated read_task/read_all_tasks
---
**Resolution:** Already done. The legacy read_task/read_all_tasks were removed in commit dc3fe41a, and KanbanLookup was collapsed to generic entity_context() calls in commit 83d3fc9e. All entity types now use the same unified code path.\n\n- [x] Replace read_task with entity_context().read()\n- [x] Replace read_all_tasks with entity_context().list()\n- [x] Remove #[allow(deprecated)] annotations\n- [x] Tests pass