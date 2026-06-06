---
name: composition
description: HookWidget over StatefulWidget, extract custom hooks, unconditional hook calls
severity: warn
---

# Dart/Flutter Composition

- Prefer `HookWidget`/`HookConsumerWidget` over `StatefulWidget` for lifecycle-dependent objects (controllers, animations).
- Extract custom hooks (functions prefixed with `use`) when the same hook combination repeats.
- **All hook calls must be unconditional and at the top level of `build`** — never inside `if`, `for`, or callbacks.
