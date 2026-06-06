---
name: riverpod-providers
description: Top-level final providers, ref.watch in build, ref.read in callbacks
severity: warn
---

# Dart/Flutter Riverpod Providers

- **Providers are top-level `final` declarations.** Never inside classes, widgets, or functions — causes memory leaks.
- **`ref.watch` in `build` only** — creates reactive subscription.
- **`ref.read` in callbacks only** — one-time read without subscription.
- **`ref.listen` for side effects** — navigation, snackbars, logging.
- `ref.read` in `build` as a "performance optimization" is explicitly wrong — makes UI go out of sync.
- `ref.watch` in a callback is wrong — value may be stale.
- Providers self-initialize. A widget calling `ref.read(provider).init()` from `initState` is an anti-pattern — initialization belongs in the provider's `build` method.
