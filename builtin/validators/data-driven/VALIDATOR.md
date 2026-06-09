---
name: data-driven
description: >-
  Flag hardcoded literals and `match`/`if`-chains over a known set that should be
  a table; repeated literals that should be a named constant. Express variation
  as data (tables, maps, config, declarative specs) interpreted by a single code
  path — not as parallel code paths a human must keep in lockstep.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: warn
---

# Data-Driven Validator

Machine-written code trends toward hardcoding — enumerating cases in control flow
and sprinkling literals — where the right shape is data interpreted by one code
path. This validator pushes the other way. It is an **in-file judgment**: it
reads the diff and needs no engine probe, so it declares none.
