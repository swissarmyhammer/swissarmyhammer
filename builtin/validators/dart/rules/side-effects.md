---
name: side-effects
description: Providers represent reads not writes, mutations in Notifier methods, onDispose cleanup
severity: warn
---

# Dart/Flutter Side Effects

- **Providers represent reads, not writes.** A `FutureProvider` whose body calls `http.post(...)` is wrong.
- Mutations belong in `Notifier` methods triggered by user actions.
- `ref.onDispose` for resource cleanup (StreamControllers, timers). No side-effect-triggering code in `onDispose`.
