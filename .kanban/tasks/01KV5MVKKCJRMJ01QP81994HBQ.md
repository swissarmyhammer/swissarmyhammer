---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
title: Wire the plan skill to adversarially double-check the board before handoff
---
## What

Update `builtin/skills/plan/SKILL.md` so that before the final handoff (the `/finish` or `/implement` reminder), the planner adversarially double-checks the produced kanban board using the `double-check` agent.

The plan skill already delegates to the `planner` agent. Add a step instructing the planner to launch the `double-check` agent via the Task tool to critique the board: are tasks right-sized, are acceptance criteria verifiable, are dependencies/ordering sound, is anything from the stated intent missing? Incorporate REVISE findings (adjust/add/reorder tasks) before reminding the user to `/finish` or `/implement`.

Optionally add `double-check` to the `planner` agent's `skills:` list for discoverability (not strictly required, since invocation is via the Task tool).

### Changes
- Add a "Double-check the board" step near the end of the plan workflow (before the handoff reminder), spawning the `double-check` agent via the Task tool against the just-created tasks.
- Keep it bounded: apply findings once, then hand off.

## Acceptance Criteria
- [ ] plan skill body has a step that spawns the `double-check` agent to critique the board before handoff
- [ ] The "board IS the plan / no markdown" and "do not ExitPlanMode / do not auto-implement" constraints are preserved
- [ ] Step is bounded (apply findings once, then remind /finish or /implement)

## Tests
- [ ] `cargo test -p swissarmyhammer-skills` passes
- [ ] Manual: a planning run ends with a double-check pass over the board before the handoff reminder #double-check-agent