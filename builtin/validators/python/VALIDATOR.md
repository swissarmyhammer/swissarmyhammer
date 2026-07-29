---
name: python
description: >-
  Python review guidelines (Hynek Schlawack school) — class design, domain
  separation, testing, error handling, logging, dependencies, API design, and
  hashing/equality idioms applied to changed Python files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.py"
---

# Python Review Validator

This validator gives language-scoped review guidance. It comes from the review skill's `PYTHON_REVIEW.md` reference. These rules add to the universal review layers. They apply only to changed Python (`.py`) files.

Each rule is an **in-file idiom judgment**. The reviewer makes this judgment by reading the diff. The validator has no engine probes.

Fix every rule that fires. Review results are pass or fail. Findings have no advisory level or severity tier.

Add a rule to this validator only if you want the reviewer to enforce it. This validator has no advisory rules.
