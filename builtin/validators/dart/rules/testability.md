---
name: testability
description: Business logic in providers/notifiers, one ProviderContainer per test, mock at service layer
severity: warn
---

# Dart/Flutter Testability

- Business logic lives in providers/notifiers, not widgets. A widget with `if/else` business logic or direct API calls is untestable.
- One `ProviderContainer` per test — never share between tests.
- Mock at the repository/service layer by overriding providers in `overrides`, not by mocking Notifiers directly.
- Widget tests wrap with `ProviderScope` and override all I/O-touching providers.
