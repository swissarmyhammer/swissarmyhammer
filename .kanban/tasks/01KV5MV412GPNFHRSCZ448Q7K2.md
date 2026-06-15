---
assignees:
- claude-code
position_column: todo
position_ordinal: a180
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
- [ ] `double-check/SKILL.md` frontmatter has `agent: double-check`
- [ ] Body includes the delegate-to-subagent partial and no longer instructs asking the user questions one at a time
- [ ] `compatibility:` typo fixed
- [ ] Description still triggers on "double check"/"verify"/"sanity check"

## Tests
- [ ] `cargo test -p swissarmyhammer-skills` passes (skill loads/parses with the `agent` field — see deploy.rs precedent asserting `agent.as_deref()`)
- [ ] Manual: after rebuild+redeploy, invoking the double-check skill launches the `double-check` agent via the Task tool and relays its verdict #double-check-agent