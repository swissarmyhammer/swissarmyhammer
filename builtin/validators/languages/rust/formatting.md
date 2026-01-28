---
name: rust-formatt  
description: Make sure rust code is formatted
severity: warn
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "**/*.rs"
tags:
  - rust
  - patterns
  - conventions
timeout: 30
---

# Rust Formatting

Run `cargo fmt` on the changed files to ensure consistent code formatting according to Rust conventions.
