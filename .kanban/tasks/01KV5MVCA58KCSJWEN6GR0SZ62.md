---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
title: Wire really-done to get adversarial sign-off from the double-check agent
---
## What

Update `builtin/skills/really-done/SKILL.md` so its completion gate includes adversarial sign-off from the `double-check` agent, in addition to the existing run-the-command evidence requirement.

really-done has NO `agent:` frontmatter (it runs inline inside whatever agent invoked it — implementer, reviewer, etc.), so it must NOT gain a delegate agent. Instead, add an explicit step in the body instructing the running agent to launch the `double-check` agent via the Task tool (`subagent_type: double-check`) for an adversarial critique, then incorporate its findings before any completion claim.

**Advisory gate (per user decision):** double-check ALWAYS runs and surfaces findings, but it does not hard-block. On `REVISE`, the caller should fix the findings; if it chooses to proceed anyway, it must record a brief justification (e.g. a task comment) rather than silently ignoring them. Evidence-before-claims (actually running the verification command) remains the HARD, non-advisory requirement.

### Changes
- Add a gate step: when there are code changes to verify, spawn the `double-check` agent (Task tool), read its PASS/REVISE verdict, and address REVISE findings before claiming done — or proceed with a logged justification.
- Keep the Iron Law / evidence-before-claims framing intact — double-check is an ADDITIONAL advisory gate, not a replacement for running verification commands.
- **Bound the loop** (review-churn lesson): act on findings and re-check at most once, then either claim done or proceed-with-justification. Do not loop indefinitely.
- Skip the double-check spawn when there is no diff (nothing to verify adversarially).

## Acceptance Criteria
- [ ] really-done body has a step that spawns the `double-check` agent via the Task tool for adversarial sign-off
- [ ] Evidence-before-claims (run the command) remains required and primary (hard, not advisory)
- [ ] double-check findings are advisory: caller may proceed past REVISE with a logged justification
- [ ] Loop is explicitly bounded (re-check at most once)
- [ ] No `agent:` field added to really-done frontmatter

## Tests
- [ ] `cargo test -p swissarmyhammer-skills` passes
- [ ] Manual: invoking really-done after a code change launches double-check and surfaces findings; proceeding past REVISE requires a logged justification