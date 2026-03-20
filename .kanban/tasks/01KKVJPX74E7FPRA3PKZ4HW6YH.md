---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9e80
title: 'WARNING: percent_complete blocks on O(n) entity list inside every GetBoard call'
---
File: swissarmyhammer-kanban/src/board/get.rs lines 173-185 — `percent_complete` is computed by counting done tasks inline inside `GetBoard::execute`. The same `all_tasks` slice is iterated twice: once via the column_counts HashMap (loop at line 88) and once via the filter at line 175 calling `task_is_ready` again. `task_is_ready` itself iterates `all_tasks` to resolve dependencies, making the full complexity O(n^2) in the number of tasks.\n\nThis happens on every board refresh (which fires on every entity event). For boards with hundreds of tasks this will be noticeable.\n\nSuggestion: compute ready/blocked counts in the single pass at line 88 where `task_is_ready` is already called per task, caching the result. The summary counts should be derived from the already-computed `column_ready_counts` map rather than a second pass.\n\nVerification step: trace lines 88-98 vs 174-178 and confirm `task_is_ready` is called twice for every task."