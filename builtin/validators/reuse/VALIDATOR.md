---
name: reuse
description: >-
  Search for an existing function before you write a new one. A near-match
  that you can extend beats a fresh copy.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - similar
---

# Reuse Validator

This validator finds a block of code that reinvents something that already
exists elsewhere. Examples: a shared utility, a standard-library function, or
an existing abstraction. The code must call the existing one instead.

The engine runs the `similar` probe (`search code`, semantic) on each added
function body. It attaches the most similar existing code as **reuse
candidates**. The `similar` probe gives a candidate, not a fact. It helps
your judgment, but it never confirms a match on its own. You decide whether
the candidate offers the same capability that the new code needs to call.
