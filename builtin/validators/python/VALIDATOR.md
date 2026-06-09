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
severity: warn
---

# Python Review Validator

Language-scoped review guidance migrated from the review skill's
`PYTHON_REVIEW.md` reference. These rules supplement the universal review
layers and apply to changed Python (`.py`) files only.

Each rule is an **in-file idiom judgment** read from the diff — there are no
engine probes. Most findings are warnings or nits; rules that the source marks
as a blocker carry `error` severity.
