---
name: rust
description: >-
  Rust review guidelines (dtolnay school). Check error handling, type safety,
  API design, trait implementations, future-proofing, and documentation style
  in changed Rust files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.rs"
---

# Rust Review Validator

This review guidance came from the review skill's `RUST_REVIEW.md` reference.
These rules add to the universal review layers. They apply only to changed
Rust (`.rs`) files.

Each rule is an **idiom judgment** read from the diff. There are no engine
probes.

You must fix every rule that fires. The review result is pass or fail. There
is no advisory tier and no severity tier among findings.

Add a rule to this validator only if you want it enforced. This validator has
no advisory rules.
