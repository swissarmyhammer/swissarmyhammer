---
name: dtolnay
description: Enforces Rust coding style inspired by dtolnay (David Tolnay) - author of serde, syn, anyhow, thiserror
version: "0.1.1"
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "**/*.rs"
tags:
  - rust
  - style
  - dtolnay
severity: error
timeout: 180
---

# dtolnay Style Rules

Enforces the Rust coding philosophy of David Tolnay (dtolnay), one of the most
influential Rust library authors. These rules distill patterns from serde, syn,
quote, anyhow, thiserror, proc-macro2, and his extensive code review history.

The core philosophy: code should be **precise, minimal, and obvious**. Every
line should earn its place. Types should make illegal states unrepresentable.
Error messages should help the user fix the problem.

Rules are automatically discovered from the `rules/` directory.
