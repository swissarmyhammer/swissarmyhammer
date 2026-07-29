---
name: data-driven
description: >-
  Flag hardcoded literals. Flag `match`/`if`-chains over a known set; these
  belong in a table instead. Flag repeated literals too; name each one as a
  constant. Express variation as data — tables, maps, config, or declarative
  specs — that a single code path interprets. Do not express variation as
  parallel code paths that a human must keep in lockstep.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
---

# Data-Driven Validator

Machine-written code tends toward hardcoding. It enumerates cases in control
flow and scatters literals through the code, where data interpreted by one
code path is the better shape. This validator pushes against that trend. It
is an **in-file judgment**. It reads the diff and needs no engine probe, so
it declares none.

** IMPORTANT ** This rule does not apply to test code.
