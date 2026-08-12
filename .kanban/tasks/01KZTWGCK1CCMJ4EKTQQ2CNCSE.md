---
assignees:
- claude-code
position_column: todo
position_ordinal: ffdb80
title: A recorded finding must name the validator set and the rule that produced it
---
A review finding recorded on a kanban task names a file and a line. It never names the validator set or the rule that produced it.

## Why this costs

An implementer who picks up a card with open findings cannot tell which rule to read. To act on a finding, or to judge whether the rule measures the right thing, the implementer must infer the rule from the wording of the finding.

Measured on 2026-08-12, unsticking ^wwb6hk7: three findings stood on the card. The rule behind the duplication finding was confirmed only by reading `builtin/validators/duplication/rules/duplication.md` and matching its carve-out text to the finding. The rule behind the other two took four searches over `builtin/validators/` to reach `completeness/rules/invariant-propagation.md`, and the attribution was never proved — only inferred from the wording.

The finish skill states that a rule which measures the wrong thing produces findings that are NOT requirements, and that a person must correct the rule. That decision needs the rule's name. Today the name is guesswork.

## What to do

- Carry the validator set name and the rule name on each finding, from the engine through to the GFM checklist a review writes on a task.
- State them in the checklist item, beside the `file:line`.
- Hold the shape with a test: a finding written to a task names a set and a rule that both exist in the loaded roster.

## Done when

- Every item of a `## Review Findings` section names its set and its rule.
- A reader can open the rule from the finding with no search.

#tool-validators