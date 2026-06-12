---
name: code-generation
description: riverpod annotations, functional vs class-based providers, params over .family
severity: warn
---

# Dart/Flutter Code Generation

- Projects already using `freezed`/`json_serializable` should use `@riverpod` annotations.
- Functional providers (annotated functions) for read-only/derived state.
- Class-based providers (annotated Notifier subclasses) for mutable state with user-triggered methods.
- Parameterized providers expressed as parameters on the annotated function/build method, not `.family` modifier syntax.
