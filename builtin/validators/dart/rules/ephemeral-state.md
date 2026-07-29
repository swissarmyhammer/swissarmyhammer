---
name: ephemeral-state
description: Providers for shared business state, hooks/StatefulWidget for widget-local state
---

# Dart/Flutter Ephemeral State

- Use providers for shared business state. Do not use providers for widget-local lifecycle concerns.
- Use `flutter_hooks` or `StatefulWidget` for form fields, animation controllers, scroll controllers, and selected-item state. Examples of `flutter_hooks` functions are `useTextEditingController()` and `useAnimationController()`.
- A `StateProvider<String>` for a text field is wrong.
