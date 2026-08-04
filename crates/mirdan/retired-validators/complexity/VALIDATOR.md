---
name: complexity
description: >-
  Flag functions the engine measured as too complex — a cognitive-complexity
  score or a condition-nesting depth over its gate. The numbers are computed
  from the parse and handed to you; compare, never count.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - complexity
---

# Complexity Validator

The engine measures this file before you read it. The `complexity` probe parses
each file under review and computes, per function, the published Sonar cognitive
complexity and the maximum condition-nesting depth, plus supporting counts. It
then lists **one row per function that is over a gate**.

That list is the finding set. Your job is to report the listed functions and say
what makes each one hard to read — not to work out which functions belong on the
list.

Do not recount. A count you do by eye disagrees with the measured number from one
run to the next, and that drift is the exact defect this probe exists to remove.
The probe's number is the fact of record.

Read the rule for what the numbers mean and what to do when the probe reports
that it could not compute them.
