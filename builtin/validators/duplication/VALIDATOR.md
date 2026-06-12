---
name: duplication
description: >-
  Flag verbatim or near-verbatim copied blocks. Machine-written code trends
  toward copy-paste; copies drift out of sync and inflate the surface area.
  Two blocks that differ only by a value are one function with an argument —
  extract a shared function and parameterize the difference.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - duplicates
severity: error
---

# Duplication Validator

Promote one of the exact problems machine-written code keeps reintroducing —
duplicated, copy-pasted code — into a first-class, focused review concern. This
validator does one thing: catch verbatim and near-verbatim copied blocks so they
become a shared function instead of N copies a human must keep in lockstep.

The engine runs the `duplicates` probe (`find duplicates` over the changed files
plus a changed-set comparison) and injects the matching blocks as ground-truth
evidence — you do not have to ask the agent to go look for duplicates, the
evidence is already on the finding.
