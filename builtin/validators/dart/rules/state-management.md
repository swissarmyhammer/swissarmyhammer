---
name: state-management
description: Notifier/AsyncNotifier, AsyncValue.guard, valueOrNull, autoDispose default
severity: warn
---

# Dart/Flutter State Management

- **`Notifier`/`AsyncNotifier`**, not deprecated `StateNotifier`/`StateNotifierProvider`.
- Initialization logic in `build()`, not constructors.
- `AsyncValue.guard()` for async error handling — not manual try/catch with `state = AsyncError(...)`.
- `state.valueOrNull` over `state.asData!` — force-unwrapping throws on loading/error.
- `autoDispose` is the correct default. Providers without listeners should not persist. `ref.keepAlive()` is the opt-in exception, and should be conditional (keep on success, dispose on error).
