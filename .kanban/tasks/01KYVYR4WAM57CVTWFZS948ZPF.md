---
assignees:
- claude-code
position_column: todo
position_ordinal: c280
title: '_partials/findings-are-requirements: one rule-obedience stance, included by every coding agent and skill'
---
# Problem

Across sessions and models, agents editorialize about rules: findings get labeled "nits", rules get weighed against their cost, and agents propose to weaken the review loop instead of obeying it. The finish skill carries strong anti-dismissal language, but only finish has it. The implementer, reviewer, tester, and the other skills do not — so the stance does not reach the agents that write the code.

The counter-stance must ship in the product, once, and render into every prompt — the same pattern as `_partials/validator-tools.md` ("You are a judge, not an editor").

# Changes

## 1. New partial: `builtin/_partials/findings-are-requirements.md`

Short, strong, declarative. Content:

- A validator rule is a requirement. A review finding is a requirement.
- There is no severity tier. Never write "nit", "minor", "cosmetic", "polish", "pedantry", or "churn" about a finding — not in reports, comments, commits, or summaries. State findings verbatim.
- Do not weigh a rule's cost. Do not skip a rule to save time or effort. Do not decide you know better than the rule.
- Satisfy a finding by removing its cause across the whole file, not only at the cited line.
- Only a true conflict — two rules that cannot both hold, or a rule that demands code that cannot compile — is reportable. Record it on the task, mark the task stuck, and stop. Never resolve it yourself and never edit a validator.

## 2. Include it everywhere code gets written or judged

- `builtin/agents/implementer/AGENT.md`
- `builtin/agents/reviewer/AGENT.md`
- `builtin/agents/tester/AGENT.md`
- `builtin/agents/committer/AGENT.md`
- `builtin/skills/implement/SKILL.md`
- `builtin/skills/finish/SKILL.md` (replace the overlapping prose in Scope with the include where the render allows; keep the loop-mechanics text)
- `builtin/skills/review/SKILL.md`

## 3. Coverage guard

A test that asserts each of the agents and skills above renders the partial's text — so a future agent or skill cannot silently drop the stance.

# Acceptance

- `cargo nextest run -E 'rdeps(swissarmyhammer-skills) or rdeps(swissarmyhammer-agents)'` passes.
- Each listed agent and skill includes `_partials/findings-are-requirements`.
- The coverage-guard test fails when an included file removes the partial. #review