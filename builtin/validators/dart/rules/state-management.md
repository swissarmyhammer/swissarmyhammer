---
name: state-management
description: Notifier/AsyncNotifier, AsyncValue.guard, valueOrNull, autoDispose default
---

# Dart/Flutter State Management

- Use `Notifier` or `AsyncNotifier`. Do not use the deprecated `StateNotifier` or `StateNotifierProvider`.
- Put initialization logic in `build()`. Do not put initialization logic in constructors.
- Use `AsyncValue.guard()` for async error handling. Do not use a manual try/catch with `state = AsyncError(...)`.
- Use `state.valueOrNull` instead of `state.asData!`. Force-unwrapping with `state.asData!` throws an error during loading or error states.
- `autoDispose` is the correct default setting. A provider without listeners must not persist. `ref.keepAlive()` is the exception. You must opt in to use `ref.keepAlive()`. Make `ref.keepAlive()` conditional. Keep the provider after success. Dispose of the provider after an error.
