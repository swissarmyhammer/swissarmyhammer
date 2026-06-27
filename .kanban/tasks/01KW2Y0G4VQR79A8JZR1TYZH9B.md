---
assignees:
- claude-code
depends_on:
- 01KW2XZS1ZK47Q888HPZ3AX4XT
position_column: doing
position_ordinal: '8280'
project: local-review
title: 'Review: purge nit/severity language + fix-at-root in skills & validator docs'
---
No "nit" / "warning" / "blocker" tiering anywhere user- or agent-facing. Every rule that fires is mandatory; a finding is a failure to be fixed at the root so it never recurs in that file.

## Purge the language
- `builtin/skills/review/SKILL.md`: counts line (`:32`), summary-by-severity (`:174`), examples (`:199`) → "N findings" / "clean". Remove any blocker/warning/nit vocabulary.
- `builtin/skills/check-sah/SKILL.md:126`: drop "fresh nits on unchanged lines."
- `builtin/validators/{rust,python,js-ts,dart}/VALIDATOR.md` (`:22`/`:31`): delete "Most findings are warnings or nits…"; replace with "every rule that fires must be fixed."

## Add fix-at-root behavior
- `builtin/skills/review/SKILL.md` + `builtin/skills/finish/SKILL.md`: a finding is satisfied by eliminating its CAUSE across the file, not by patching the cited line — so re-review of that file finds zero recurrences. Frame review as binary (pass/fail, like the test suite): any open finding ⇒ not done.
- `finish/SKILL.md`: the loop already runs to zero findings; reword away from severity, keep the 3-iteration stuck guardrail.

## Validator authoring guidance
Note in validator docs: only put a rule in a validator if you want it ENFORCED. There are no advisory rules.

REMINDER: edit `builtin/skills/**` and `builtin/validators/**` — NOT generated `.skills/`. Depends on the severity-removal card (counts shape drives the skill wording).