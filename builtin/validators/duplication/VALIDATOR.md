---
name: duplication
description: >-
  Flag verbatim or near-verbatim copied blocks. Machine-written code trends
  toward copy-paste. Copies drift out of sync. Copies inflate the surface
  area. Two blocks that differ only by a value are one function with an
  argument. Extract a shared function. Parameterize the difference.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - duplicates
---

# Duplication Validator

Machine-written code keeps reintroducing one exact problem: duplicated,
copy-pasted code. This validator promotes that problem into a first-class,
focused review concern. This validator does one thing. It catches verbatim
and near-verbatim copied blocks. The fix turns the copies into a shared
function, instead of leaving N copies that a human must keep in lockstep.

The engine runs the `duplicates` probe. This probe runs `find duplicates`
over the changed files, plus a changed-set comparison. The engine injects
the matching blocks as ground-truth evidence. You do not need to ask the
agent to look for duplicates. The evidence is already on the finding.

**IMPORTANT**: This rule does not apply to test code.
