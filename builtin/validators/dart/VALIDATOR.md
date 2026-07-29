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

This guidance comes from the review skill's `DART_FLUTTER_REVIEW.md` reference. These rules add to the universal review layers. These rules apply only to changed Dart (`.dart`) files.

Each rule gives an in-file idiom judgment. You read the judgment from the diff. The validator does not use engine probes. You must fix every rule that fires. The review gives one of two results: pass or fail. No finding has an advisory level or a severity tier. Add a rule to this validator only if you want the review to enforce it. This validator has no advisory rules.
