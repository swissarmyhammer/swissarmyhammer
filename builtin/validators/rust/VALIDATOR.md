---
name: rust
description: >-
  Rust review guidelines (dtolnay school) — error handling, type safety, API
  design, trait impls, future-proofing, and documentation idioms applied to
  changed Rust files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.rs"
severity: warn
---

# Rust Review Validator

Language-scoped review guidance migrated from the review skill's
`RUST_REVIEW.md` reference. These rules supplement the universal review layers
and apply to changed Rust (`.rs`) files only.

Each rule is an **in-file idiom judgment** read from the diff — there are no
engine probes. Most findings are warnings or nits; rules that the source marks
as a blocker carry `error` severity.
