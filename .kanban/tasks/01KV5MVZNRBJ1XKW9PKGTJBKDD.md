---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
title: Make the implement skill run really-done before moving a task to review
---
## What

Update `builtin/skills/implement/SKILL.md` so the implementer runs the `really-done` gate (which now includes the advisory adversarial double-check) before moving a task into the `review` column.

Currently step 6 is "Move to review" once subtasks are checked. Insert a verification step before it: invoke `really-done` to verify the work — run verification commands (hard requirement) AND get the double-check agent's advisory sign-off. The verification-command pass is required before moving to review; double-check findings are advisory (fix them, or proceed with a logged justification per really-done's contract).

The `implementer` agent already lists `really-done` in its `skills:`, so no agent change is needed — this is a skill-body step plus a Rules line.

### Changes
- Insert step "5.5 Verify with really-done" between Implement (5) and Move to review (6): run really-done; the run must complete (verification commands green) before moving to review; act on or log double-check findings.
- Add a Rules bullet: do not move to review until really-done has been run.
- Keep "stop after moving to review" and "no worktrees" intact.

## Acceptance Criteria
- [ ] implement skill body runs really-done before "move to review"; verification-command pass gates the move, double-check findings are advisory
- [ ] really-done's double-check sign-off is reached transitively (no direct double-check spawn from implement)
- [ ] Existing invocation/filter-DSL and review-column sections unchanged

## Tests
- [ ] `cargo test -p swissarmyhammer-skills` passes
- [ ] Manual: an implement run does not move a task to review until really-done has been run (verification green; double-check findings addressed or logged)