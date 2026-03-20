---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffd380
title: Parallel implementation in implement-loop
---
Upgrade `/implement-loop` to run independent kanban cards concurrently instead of one at a time.

**How it works:**
1. Query all ready tasks (no incomplete dependencies) instead of just `next task`
2. Spawn parallel implementer subagents for each ready card
3. Wait for all to complete
4. Query next batch of ready tasks (newly unblocked by completed dependencies)
5. Repeat until board is clear

**Already have:**
- `depends_on` field on kanban tasks
- `ready` filter on task queries (returns only unblocked tasks)
- Implementer subagent infrastructure
- `/implement-loop` skill as the orchestration point

**Need to build:**
- Batch-ready-task query in implement-loop skill
- Parallel subagent spawning (multiple `Agent` calls in one message)
- Result collection and error handling (if one card fails, continue others? stop wave?)
- Concurrency limit (don't spawn 20 agents at once)