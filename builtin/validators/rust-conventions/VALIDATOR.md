---
name: rust-conventions
description: Rust language patterns and conventions validation
version: 1.0.0
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
severity: warn
timeout: 30
---

# Rust Conventions RuleSet

Validates Rust code follows language-specific patterns, conventions, and best practices.

This RuleSet evaluates Rust code for:
- Language patterns and idiomatic code
- Code formatting and style
- Testing conventions

Rules in this RuleSet have warn severity (non-blocking guidance).
