---
assignees:
- claude-code
position_column: todo
position_ordinal: a580
title: Rebuild, redeploy, and verify the double-check agent end to end
---
## What

Build the new agent into the binary, redeploy the updated skills + agent, and verify the whole chain works against a real model.

The `double-check` agent is embedded at compile time by `swissarmyhammer-agents/build.rs`, and skills/agents deploy to `~/.skills` / `~/.agents` via the mirdan store. Per the deploy lesson: a rebuild alone leaves the old deployed copy live — both steps are required.

### Steps
- `just sah` — rebuild the binary so build.rs embeds `builtin/agents/double-check/` and the updated skill bodies.
- `sah init` — redeploy to `~/.agents` and `~/.skills` (the editor agent/skill dirs symlink into the store).
- Confirm `double-check` appears in the resolved agent list and the double-check skill carries `agent: double-check`.
- Real-model end-to-end (per the fake-agent-must-match-contract lesson, verify with a real shell-out, not a scripted fake):
  - Make a trivial deliberately-flawed change, invoke the double-check skill, confirm the agent returns a REVISE verdict with actionable findings and does NOT ask the user a question.
  - Confirm an `/implement` run reaches really-done → double-check before moving to review.

## Acceptance Criteria
- [ ] `just sah` and `sah init` complete cleanly
- [ ] Resolved agents include `double-check`; deployed double-check skill has `agent: double-check`
- [ ] Real-model run: double-check returns a structured PASS/REVISE verdict and never prompts the user
- [ ] implement → really-done → double-check chain observed before review

## Tests
- [ ] Full `cargo test` for swissarmyhammer-agents and swissarmyhammer-skills green (zero failures, zero warnings)
- [ ] `/code-review` or `review working` clean on the diff
- [ ] Documented evidence of the real-model double-check run (transcript / output) #double-check-agent