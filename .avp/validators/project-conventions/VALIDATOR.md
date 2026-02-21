---
name: project-conventions
description: Project-specific coding conventions for swissarmyhammer
metadata:
  version: "1.0.0"
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "**/*.rs"
tags:
  - project
  - conventions
severity: error
timeout: 30
---

# Project Conventions RuleSet

Enforces swissarmyhammer project-specific coding standards that go beyond general Rust conventions.
