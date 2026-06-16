---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb480
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
- [x] plan skill body has a step that spawns the `double-check` agent to critique the board before handoff
- [x] The "board IS the plan / no markdown" and "do not ExitPlanMode / do not auto-implement" constraints are preserved
- [x] Step is bounded (apply findings once, then remind /finish or /implement)

## Tests
- [x] `cargo test -p swissarmyhammer-skills` passes
- [x] Manual: a planning run ends with a double-check pass over the board before the handoff reminder #double-check-agent

## Progress (2026-06-15)

Done.
- `builtin/skills/plan/SKILL.md`: added a "Double-check the board" constraint subsection placed at the end of the workflow, immediately before the handoff constraints. It launches the `double-check` agent via the Task tool (`subagent_type: double-check`) to critique the board (right-sized tasks, verifiable acceptance criteria, sound dependencies/ordering, missing intent), then applies REVISE findings ONCE before the `/finish`/`/implement` reminder. Also added it as step 6 in the Example numbered list (handoff renumbered to 7). The double-check note reiterates "the board IS the plan; never produces or reads a markdown plan file."
- Preserved verbatim: "The board IS the plan / Never write a markdown plan file", "Do NOT call `ExitPlanMode`", and the "No auto-implementation on exit" constraint.
- `builtin/agents/planner/AGENT.md`: added `double-check` to the `skills:` list for discoverability.

Verification (fresh): grep confirms the double-check step (`subagent_type: double-check`), the board-IS-the-plan/no-markdown text, the no-ExitPlanMode/no-auto-implement constraints, and bounded "once" language all present. `cargo test -p swissarmyhammer-skills` → `test result: ok. 114 passed; 0 failed` (+ 2 + 2 + 0 doctest, exit 0, no warnings). `cargo test -p swissarmyhammer-agents` → `test result: ok. 109 passed; 0 failed` (exit 0, no warnings).