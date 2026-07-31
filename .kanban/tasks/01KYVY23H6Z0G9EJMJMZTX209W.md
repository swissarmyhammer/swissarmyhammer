---
assignees:
- claude-code
depends_on:
- 01KYVYC5DE70XGP2P9GT6TW0KG
position_column: todo
position_ordinal: c080
title: 'Implement skill: read validator rules before coding, self-review before handoff'
---
# Problem

The implementer never reads the validator rules. The review engine then finds the rule violations after the code is written. Each fix pass adds new code that also does not obey the rules. Session 4203e383 used 8 review iterations for one task (^1t92gnj). More than 7 hours went to convergence after the first implementation.

The fix is to follow the rules from the start. Do not weaken the review gate.

Depends on ^t6tw0kg: the skill uses one `rules: true` call per file. A loop of `get validator` calls is an opportunity to fail — do not use it.

Related: ^s948zpf adds the `findings-are-requirements` partial that carries the reporting-language stance (findings verbatim, no severity words) into every agent and skill.

# Changes

## 1. `builtin/skills/implement/SKILL.md` — add a "Know the rules" step

Add to the "Research before writing" section:

- Before you edit a file, get the rules that review will enforce on it — one call: `{"op": "list validators", "match": "<file path>", "rules": true}` on the `review` tool. The response carries every applicable rule body verbatim.
- Obey each rule when you write the code, not after: document each public item, name each numeric constant, do not copy blocks, keep functions small and flat, follow the project naming, delete dead code.

## 2. `builtin/skills/implement/SKILL.md` — add a "Self-review" step

Add a new step before the `/double-check` step:

- Run `{"op": "review working"}` on your changes.
- Fix every finding. A finding is a requirement. Do not rank findings. Do not defer findings. Do not label findings.
- Run the review again. Repeat until the review is clean.
- Only then hand off for the formal `/review`.

Rationale: one author-side review run costs ~15 minutes. One full implement→test→review iteration costs ~50 minutes. The self-review replaces iterations.

# Acceptance

- `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'` passes.
- The implement skill instructs the agent to fetch the applicable rules with one `rules: true` call per file before editing, and to run `review working` until clean before handoff.