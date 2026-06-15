---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb180
title: Rewrite the double-check skill to delegate to the double-check agent
---
## What

Rewrite `builtin/skills/double-check/SKILL.md` so the skill delegates to the new `double-check` agent instead of asking the user clarifying questions.

### Changes
- Add `agent: double-check` to the frontmatter (mirrors `plan: planner` and `implement: implementer`).
- Add `{% include "_partials/delegate-to-subagent" %}` to the body so the work runs in the Task-launched agent (which inherits `code_context`/`git` MCP tools, unlike a `context: fork`).
- **Remove step 3 ("Clarify — make a numbered list and ask one at a time").** The new contract is adversarial feedback returned to the caller, not questions to the user. Reframe the body around: run the adversarial double-check, then act on the returned PASS/REVISE findings.
- Fix the stray `x` typo on the `compatibility:` frontmatter line (`tool x for ... work.x` → clean text).

## Acceptance Criteria
- [x] `double-check/SKILL.md` frontmatter has `agent: double-check`
- [x] Body includes the delegate-to-subagent partial and no longer instructs asking the user questions one at a time
- [x] `compatibility:` typo fixed
- [x] Description still triggers on "double check"/"verify"/"sanity check"

## Tests
- [x] `cargo test -p swissarmyhammer-skills` passes (skill loads/parses with the `agent` field — see deploy.rs precedent asserting `agent.as_deref()`)
- [ ] Manual: after rebuild+redeploy, invoking the double-check skill launches the `double-check` agent via the Task tool and relays its verdict #double-check-agent

---

## Implementation Summary (2026-06-15)

Rewrote `builtin/skills/double-check/SKILL.md`:
- Added `agent: double-check` to frontmatter (line 6), matching plan/implement placement.
- Added `{% include "_partials/delegate-to-subagent" %}` after the heading (line 16).
- Removed the old step 3 "Clarify — ask one at a time" and reframed the Process around handing the change+intent to the adversarial `double-check` agent and acting on its returned `VERDICT: PASS`/`VERDICT: REVISE` findings (no user questions).
- Fixed the `compatibility:` line: removed the stray `tool x` / `work.x` typos.

Verification (direct, fresh):
- `grep -n "agent: double-check"` → line 6
- `grep -n "delegate-to-subagent"` → line 16
- `grep "ask one at a time"` → no match (removed)
- `grep "tool x\|work.x"` → no match (typo fixed)
- `cargo test -p swissarmyhammer-skills` → test result: ok. 114 passed; 0 failed (+2+2+0 in other test binaries), 0 warnings.