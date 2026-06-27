---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3d7y1hz20vkqpvmt83vaa0
  text: |-
    Done. Purged severity/nit vocabulary and added fix-at-root + binary pass/fail framing across builtin docs.

    Files changed (builtin only — generated .skills/ untouched):
    - builtin/skills/review/SKILL.md — counts/summary/examples were already on {findings,confirmed,refuted}/"N findings"/"clean" from the z3ax4xt card; added a new Rules bullet "Fix at the root, not the cited line" (eliminate the cause across the whole file so re-review finds zero recurrences; review is binary like a test suite, any open finding ⇒ not done, no severity tier makes a finding optional/advisory).
    - builtin/skills/finish/SKILL.md — reworded Scope step "Fix the code" → "Fix the code at the root" with the same fix-at-root + binary framing; kept the 3-iteration stuck guardrail (step 7) intact.
    - builtin/skills/check-sah/SKILL.md — review-churn bullet: dropped "fresh nits on unchanged lines" → "fresh findings on unchanged lines"; also fixed stale severity vocab "rising blocker `counts`" → "rising finding `counts`".
    - builtin/validators/{rust,python,js-ts,dart}/VALIDATOR.md — deleted "Most findings are warnings or nits; …carry `error` severity"; replaced with: "Every rule that fires must be fixed — review is binary pass/fail, with no advisory or severity tier among findings. Only add a rule to this validator if you want it enforced; there are no advisory rules." (covers both the purge and the validator-authoring guidance).

    Out of scope / intentionally left: per-rule `severity:` YAML frontmatter (functional wire config read by the engine) and "blocker" enforcement language in injection/command-safety/no-secrets/duplication validators (describes what those validators enforce, not a finding-severity tier).

    Verification:
      grep -rin -E '\bnit(s)?\b' builtin/skills builtin/validators  → no output (exit 1)
      grep -rin -E 'warnings or nits|Most findings are' builtin/skills builtin/validators → no output (exit 1)

    git status: only the 7 builtin/ files modified; no .skills/ files, no Cargo/heb changes touched.
  timestamp: 2026-06-27T02:06:22.769992+00:00
depends_on:
- 01KW2XZS1ZK47Q888HPZ3AX4XT
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe680
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