---
name: swift
description: >-
  Swift review guidelines from Apple's API Design Guidelines and Apple's
  open-source libraries (stdlib, swift-nio, swift-argument-parser,
  swift-collections, swift-format) — casing, naming clarity, doc parameter
  naming, fluent usage, idioms, value semantics, access control, error
  handling, optionals, concurrency, and state modeling applied to changed
  Swift files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.swift"
---

# Swift Review Validator

Language-scoped review guidance for changed Swift (`.swift`) files, grounded in
two sources: Apple's **Swift API Design Guidelines** and the idioms of Apple's
own **open-source Swift** projects.

Each rule is an **in-file idiom judgment** read from the diff — there are no
engine probes. Every rule that fires must be fixed — review is binary
pass/fail, with no advisory or severity tier among findings. Only add a rule to
this validator if you want it enforced; there are no advisory rules.

Formatting-only concerns (whitespace, indentation, import ordering, semicolons)
belong to `swift-format`, not this validator; the rules here are semantic.

Every rule here reads plain Swift. No rule is scoped to a third-party library,
so none opens with a detection clause.
