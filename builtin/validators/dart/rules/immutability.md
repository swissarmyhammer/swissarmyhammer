---
name: immutability
description: freezed/sealed classes, copyWith, union types for state variants
---

# Dart/Flutter Immutability

- Use `@freezed` or Dart 3 sealed classes for data and model classes. A mutable class that holds domain state is a warning sign.
- Use `copyWith` to change a model object. Do not mutate a field on a model object directly.
- Use union types for state variants. Use multiple factory constructors in `@freezed` classes, or in sealed classes with pattern matching. Do not use a single mutable class with `bool isLoading`, `T? data`, and `String? error` fields.
- Use Dart 3 `switch` expressions and pattern matching. Do not use the older `.when` or `.map` helpers.
- If a developer hand-writes `==`, `hashCode`, or `toString` on a data class, the developer must use `@freezed` instead.
