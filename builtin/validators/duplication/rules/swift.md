---
name: swift
description: Swift-specific duplication carve-outs for dispatch-forced override shims and conformance boilerplate
---

# Swift Duplication Carve-outs

This rule applies only to Swift files in scope. If no file in scope is
Swift, this rule contributes nothing.

Swift's dispatch rules force certain one-line methods to repeat per class.
These repeated lines are not duplication. Do not flag them:

- **Identical one-line `override` methods that forward into shared code.**
  Each method calls `super`, an already-extracted shared helper, or both.
  The helper is typically a protocol-extension method. The Swift language
  prevents hoisting these methods. All four escape routes are closed:
  - A protocol extension cannot call `super`.
  - You cannot declare `override` methods in extensions.
  - Classes with different superclasses cannot share a common base to host
    the override.
  - A member "extracted" into a protocol extension never enters class
    dynamic dispatch. The override silently stops being called. This
    change in dispatch changes the program's behavior.

  The shared logic may already live in one place, for example in a
  protocol extension. If only the per-class forwarding override repeats,
  the duplication is resolved. Do not demand further extraction. Further
  extraction would ask for code that Swift cannot express.

- **Trivial conformance stubs that the compiler requires per type.**
  Examples are a one-line `description`, `id`, or `CodingKeys`
  declaration. The body carries no logic that could drift.

Apply this test: does the repeated block contain logic that could drift out
of sync? A forwarding line, such as `super.freeze(); adapted(x)`, cannot
drift. The logic it forwards to is already shared. Flag the copies only
when actual logic repeats.
