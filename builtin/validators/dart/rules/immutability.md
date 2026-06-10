---
name: immutability
description: freezed/sealed classes, copyWith, union types for state variants
severity: warn
---

# Dart/Flutter Immutability

- **Data/model classes use `@freezed` or Dart 3 sealed classes.** Mutable classes holding domain state are a red flag.
- `copyWith` for modifications, never direct field mutation on model objects.
- **Union types for state variants.** Use multiple factory constructors in `@freezed` or sealed classes with pattern matching — not `bool isLoading + T? data + String? error` on a single mutable class.
- Use Dart 3 `switch` expressions and pattern matching over the older `.when`/`.map` helpers.
- If a developer hand-writes `==`, `hashCode`, `toString` on a data class, they should be using `@freezed`.
