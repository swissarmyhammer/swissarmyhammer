---
name: composition
description: HookWidget over StatefulWidget, extract custom hooks, unconditional hook calls
---

# Dart/Flutter Composition

- Use `HookWidget` or `HookConsumerWidget` instead of `StatefulWidget` for lifecycle-dependent objects, such as controllers and animations.
- Extract a custom hook when the same hook combination repeats. Prefix the hook function name with `use`.
- Call every hook unconditionally, at the top level of `build`. Do not call a hook inside `if`, inside `for`, or inside a callback.
