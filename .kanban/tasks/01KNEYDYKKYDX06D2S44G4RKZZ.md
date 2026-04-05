---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: 'WARNING: GetBoard still uses task_is_ready directly instead of enrichment pipeline'
---
**File:** swissarmyhammer-kanban/src/board/get.rs\n\n**What:** `GetBoard::execute` computes ready counts by calling `task_is_ready` directly in a per-task loop, while `ListTasks` and `NextTask` use the new `enrich_all_task_entities` pipeline which includes virtual tags. This means the board summary's `ready_tasks` / `blocked_tasks` counts use a different code path than the task list's `ready` field.\n\n**Why this matters:** If `task_is_ready` and `ReadyStrategy` ever diverge further (they already do on missing deps -- see the blocker finding), the board summary counts will disagree with what `list tasks` reports. The board also misses the opportunity to include virtual tag metadata in its response.\n\n**Suggestion:** Refactor `GetBoard` to use `enrich_all_task_entities` for consistency, then read the pre-enriched `ready` field from each entity instead of calling `task_is_ready`. This unifies the ready-computation code path across all commands.\n\n**Verification:** cargo test -p swissarmyhammer-kanban -- board::get" #review-finding