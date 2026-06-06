---
name: magic-numbers
description: >-
  Flag unexplained numeric literals — hardcoded timeouts, limits, buffer sizes,
  ports, status codes, ratios, and retry counts — that should be named constants.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: info
---

# Magic Numbers Validator

Re-homed from the monolithic code-quality set into a focused, one-concern
validator: unexplained numeric literals. It is an **in-file judgment** — it reads
the diff and needs no engine probe, so it declares none.

This is the narrow "name your literals" concern; the broader push toward
expressing variation as data (tables/config rather than control flow) lives in
the `data-driven` validator.
