---
name: testability
description: Business logic in providers/notifiers, one ProviderContainer per test, mock at service layer
---

# Dart/Flutter Testability

- Put business logic in providers or notifiers. Do not put business logic in widgets. A widget with `if/else` business logic is not testable. A widget with direct API calls is also not testable.
- Use one `ProviderContainer` for each test. Do not share a `ProviderContainer` between tests.
- Mock at the repository or service layer. Override providers in `overrides` to do this. Do not mock Notifiers directly.
- Wrap widget tests with `ProviderScope`. Override every provider that touches I/O.
