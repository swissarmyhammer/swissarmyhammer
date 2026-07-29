---
name: riverpod-providers
description: Top-level final providers, ref.watch in build, ref.read in callbacks
---

# Dart/Flutter Riverpod Providers

- Declare a provider as a top-level `final` declaration. Do not declare a provider inside a class, a widget, or a function. A provider inside a class, a widget, or a function causes a memory leak.
- Use `ref.watch` only in `build`. `ref.watch` creates a reactive subscription.
- Use `ref.read` only in callbacks. `ref.read` performs a one-time read with no subscription.
- Use `ref.listen` for side effects, such as navigation, snackbars, and logging.
- Do not use `ref.read` in `build` as a performance optimization. This use is wrong. It makes the UI go out of sync.
- Do not use `ref.watch` in a callback. This use is wrong. The value may be stale.
- A provider must self-initialize. Do not call `ref.read(provider).init()` from `initState` in a widget. This is an anti-pattern. Initialization belongs in the provider's `build` method.
