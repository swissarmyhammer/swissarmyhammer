---
position_column: done
position_ordinal: z00
title: 'WARNING: implement-loop has no explicit termination check — could loop forever if /implement never clears cards'
---
builtin/skills/implement-loop/SKILL.md:26\n\nThe skill says: \"Only call `ralph` with `op: \"clear ralph\"` when `kanban` `next task` returns no cards.\" However, there is no explicit loop-back instruction to re-query `next task` after each `/implement` call completes. The flow is: set ralph → delegate to /implement → delegate to /test → ???. The skill does not state when or how to loop back to the `next task` check. A literal reading leaves the agent unsure whether to keep calling `/implement` or re-query kanban first.\n\nSuggestion: Add an explicit loop step: after `/test` passes, call `kanban` `next task` again; if cards remain, continue; if none, clear ralph and summarise." #review-finding