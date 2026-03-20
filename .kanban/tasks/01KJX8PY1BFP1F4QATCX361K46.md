---
position_column: done
position_ordinal: ffe480
title: O(N^2) task enrichment in list_entities and get_entity
---
In `commands.rs`, `list_entities` clones the entire task list (`entities.clone()`) and then calls `enrich_task_entity` for each task. Each enrichment call iterates `all_tasks` for `task_blocks` and `task_blocked_by`, giving O(N^2) complexity. For `get_entity`, it fetches ALL tasks just to enrich a single one. This will be a performance bottleneck for boards with hundreds of tasks.

Consider: (1) pre-building a dependency index `HashMap<String, Vec<String>>` for blocks/blocked_by lookups, or (2) for `get_entity`, only fetching the dependency chain rather than all tasks.