---
name: code-generation
description: riverpod annotations, functional vs class-based providers, params over .family
---

# Dart/Flutter Code Generation

- If a project already uses `freezed` or `json_serializable`, use `@riverpod` annotations.
- Use functional providers, which are annotated functions, for read-only or derived state.
- Use class-based providers, which are annotated Notifier subclasses, for mutable state with user-triggered methods.
- Express a parameterized provider as parameters on the annotated function or build method. Do not use `.family` modifier syntax.
