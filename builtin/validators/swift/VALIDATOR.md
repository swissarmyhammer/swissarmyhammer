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

This validator reviews changed Swift (`.swift`) files. It uses three sources
for its guidance:

- Apple's **Swift API Design Guidelines**.
- The idioms of Apple's own **open-source Swift** projects.
- The **Point-Free** functional and dependency-injection school.

Each rule makes an in-file idiom judgment. The reviewer reads this judgment
from the diff. The validator uses no engine probes.

You must fix every rule that fires. The review result is pass or fail.
Findings carry no advisory level or severity tier. Add a rule to this
validator only when you want the review to enforce it. This validator has no
advisory rules.

Formatting rules do not belong in this validator. This includes whitespace,
indentation, import order, and semicolons. Use `swift-format` for these
instead. The rules here address semantics.

Some rule files are library-conditional. Each of these files opens with a
detection clause. Apply the rule only when the changed file uses that
library. This applies to the controlled-dependency rules and the Composable
Architecture rules. Skip these rules for files that do not use the library.
