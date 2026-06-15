---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb280
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
- [x] really-done body has a step that spawns the `double-check` agent via the Task tool for adversarial sign-off
- [x] Evidence-before-claims (run the command) remains required and primary (hard, not advisory)
- [x] double-check findings are advisory: caller may proceed past REVISE with a logged justification
- [x] Loop is explicitly bounded (re-check at most once)
- [x] No `agent:` field added to really-done frontmatter

## Tests
- [x] `cargo test -p swissarmyhammer-skills` passes
- [ ] Manual: invoking really-done after a code change launches double-check and surfaces findings; proceeding past REVISE requires a logged justification #double-check-agent

## Progress (2026-06-15, claude-code)
Added an **Adversarial Sign-Off (advisory gate)** section to `builtin/skills/really-done/SKILL.md`, placed immediately after **The Gate** so the Iron Law / evidence-before-claims hard requirement stays primary and fully intact. The new section: spawns the `double-check` agent via the Task tool (`subagent_type: double-check`), reads its PASS/REVISE verdict, allows the caller to fix findings OR proceed past REVISE with a brief logged justification, bounds the loop to re-check at most once, and skips the spawn when there is no diff. Also added a discoverability line to the "When to Apply" list. No `agent:` field added to the frontmatter (verified by grep — no match). 

Verification evidence:
- grep confirmed: `subagent_type: double-check` + Task tool step present; Iron Law / "NO COMPLETION CLAIMS WITHOUT FRESH VERIFICATION EVIDENCE" intact; "advisory" + "logged justification" present; "at most once" present; no `^agent:` line.
- `cargo test -p swissarmyhammer-skills`: `test result: ok. 114 passed; 0 failed` (+ 2 passed, 2 passed, 0 doc-tests), 0 warnings, 0 errors. Includes the builtin description compliance and skill comment guidance integration tests that embed the builtin skill.