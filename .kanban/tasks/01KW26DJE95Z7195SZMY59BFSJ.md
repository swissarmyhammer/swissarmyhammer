---
assignees:
- claude-code
depends_on:
- 01KW26ART916Q6N6JX037Q4QSX
position_column: todo
position_ordinal: c280
project: expect
title: 'Authoring skill: proactively capture expectations from chat'
---
## What
The agent-layer behavior that watches a conversation and proactively offers to capture acceptance-criteria-shaped statements as expectations, then calls `expect expectation create`. Per `ideas/expect.md` §"expect expectation create" (chat source) — recognizing intent mid-conversation is the agent's job, not the tool's.

- Author a new skill source under `builtin/skills/expect/SKILL.md` (NOTE: `.skills/` is generated — edit the builtin source, per project conventions):
  - Triggers: when the user states an acceptance-criteria-shaped intent ("the coupon should only apply once", "X must do Y"), offer to capture it as an expectation.
  - On accept, invoke the `expect expectation create --from-chat` op with the mined intent + bounded (~3-5) criteria.
  - Push for the right-reason / negative-case criteria (Open Question 5: agents are weak at failure/edge scenarios — the skill explicitly prompts for "and it does NOT do X").
  - Push for invariants over frozen literals (domain-language "how things should be").
  - Hand off: drafted spec is left `new` (unapproved) for the human to edit-for-intent and `approve`.
- Keep it consistent with existing skill conventions (description quality, progressive disclosure). Regenerate `.skills/` via the project's generation step.

## Acceptance Criteria
- [ ] `builtin/skills/expect/SKILL.md` exists with a clear description and triggers; the generated `.skills/expect/` is produced by the generator (not hand-edited).
- [ ] The skill instructs invoking `expect expectation create --from-chat` and leaving the result unapproved.
- [ ] The skill explicitly elicits negative/edge criteria and invariants.

## Tests
- [ ] A skill-presence/lint test (mirror existing skills-guide compliance tests) asserts the expect skill loads with required frontmatter and a non-empty description.
- [ ] If the repo has a skill-generation check, assert `.skills/expect/` matches the builtin source.

## Workflow
- Use `/tdd` where a test harness exists for skills; otherwise validate via the skills lint/generation check.