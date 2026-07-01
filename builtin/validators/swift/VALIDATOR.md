---
name: swift
description: >-
  Swift review guidelines from Apple's API Design Guidelines, Apple's
  open-source libraries (stdlib, swift-nio, swift-argument-parser,
  swift-collections, swift-format), and the Point-Free school (Composable
  Architecture, swift-dependencies) — casing, naming clarity, fluent usage,
  value semantics, access control, error handling, optionals, concurrency,
  documentation, state modeling, and controlled dependencies applied to changed
  Swift files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.swift"
---

# Swift Review Validator

Language-scoped review guidance for changed Swift (`.swift`) files, grounded in
three sources: Apple's **Swift API Design Guidelines**, the idioms of Apple's
own **open-source Swift** projects, and the **Point-Free** functional /
dependency-injection school.

Each rule is an **in-file idiom judgment** read from the diff — there are no
engine probes. Every rule that fires must be fixed — review is binary
pass/fail, with no advisory or severity tier among findings. Only add a rule to
this validator if you want it enforced; there are no advisory rules.

Formatting-only concerns (whitespace, indentation, import ordering, semicolons)
belong to `swift-format`, not this validator; the rules here are semantic.

Some rule files are **library-conditional** — they open with a detection clause
and apply only when the changed file uses that library (the controlled-
dependency and Composable Architecture rules). Skip them for files that don't.
