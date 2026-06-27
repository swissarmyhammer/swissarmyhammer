---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4rh3cmjzdqh3326qg4bgr1
  text: 'Picked up. Research: skills live in builtin/skills/<name>/SKILL.md and are embedded at COMPILE time via crates/swissarmyhammer-skills/build.rs (BuiltinGenerator -> get_builtin_skills). There is NO .skills/ directory in this repo to hand-edit; "regenerate" == cargo build (build.rs re-embeds). Required frontmatter: name + description (validate_frontmatter); description must be <=1024 chars, no angle brackets. Harness: tests/builtin_description_compliance.rs iterates all builtins; tests/*_guidance.rs use tests/common/mod.rs rendered_builtin_instructions(). MCP op is `create expectation` -> CLI `expect expectation create` with `from_chat` param (--from-chat). Plan: TDD new test tests/expect_capture_guidance.rs, then author builtin/skills/expect/SKILL.md.'
  timestamp: 2026-06-27T14:42:51.924593+00:00
- actor: claude-code
  id: 01kw4rwhbxsy4d86h7vfk7prtk
  text: 'DONE (green, left in doing for review). Authored builtin/skills/expect/SKILL.md (frontmatter matches sibling task/really-done shape: name/description/license/compatibility/metadata, no invented fields; description is a strong retrieval hook, guide-compliant). Body: offers capture on acceptance-criteria-shaped intent, invokes `expect expectation create --from-chat` with bounded ~3-5 criteria, explicitly elicits negative/edge ("and it does NOT do X") + right-reason (401-vs-200), pushes invariants over frozen literals, leaves spec unapproved (new) for human to edit-for-intent and `observation approve`. Skills are embedded at COMPILE time via crates/swissarmyhammer-skills/build.rs (BuiltinGenerator); "regeneration" == cargo build re-embeds — no .skills/ dir exists to hand-edit. TDD: added crates/swissarmyhammer-skills/tests/expect_capture_guidance.rs (watched RED "builtin skill ''expect'' should exist" -> GREEN). Verified: cargo nextest run -p swissarmyhammer-skills = 124 passed; cargo nextest run -E ''test(skill)'' = 193 passed (incl templating all-skills-render + mirdan frontmatter-valid + description compliance); cargo check --workspace ok; cargo fmt applied. double-check agent verdict: PASS (tightened the one weak marker it flagged: "approve" -> "observation approve").'
  timestamp: 2026-06-27T14:49:06.685194+00:00
depends_on:
- 01KW26ART916Q6N6JX037Q4QSX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8380
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