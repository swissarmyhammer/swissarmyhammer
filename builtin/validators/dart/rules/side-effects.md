---
name: side-effects
description: Providers represent reads not writes, mutations in Notifier methods, onDispose cleanup
---

# Dart/Flutter Side Effects

- A provider represents a read action. A provider must not represent a write action. A `FutureProvider` with a body that calls `http.post(...)` is wrong.
- Put mutations in `Notifier` methods. A user action must trigger the method.
- Use `ref.onDispose` to clean up resources, such as StreamControllers and timers. Do not put side-effect code in `onDispose`.
