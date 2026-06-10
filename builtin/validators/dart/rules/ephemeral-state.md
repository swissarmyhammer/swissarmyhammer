---
name: ephemeral-state
description: Providers for shared business state, hooks/StatefulWidget for widget-local state
severity: warn
---

# Dart/Flutter Ephemeral State

- **Providers are for shared business state**, not widget-local lifecycle concerns.
- Form fields, animation controllers, scroll controllers, selected-item state: use `flutter_hooks` (`useTextEditingController()`, `useAnimationController()`) or `StatefulWidget`.
- A `StateProvider<String>` for a text field is wrong.
