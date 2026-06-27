---
name: dart
description: >-
  Dart/Flutter review guidelines (Remi Rousselet school) — immutability,
  Riverpod providers, state management, ephemeral state, side effects, code
  generation, composition, and testability idioms applied to changed Dart files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.dart"
---

# Dart/Flutter Review Validator

Language-scoped review guidance migrated from the review skill's
`DART_FLUTTER_REVIEW.md` reference. These rules supplement the universal
review layers and apply to changed Dart (`.dart`) files only.

Each rule is an **in-file idiom judgment** read from the diff — there are no
engine probes. Every rule that fires must be fixed — review is binary
pass/fail, with no advisory or severity tier among findings. Only add a rule to
this validator if you want it enforced; there are no advisory rules.
