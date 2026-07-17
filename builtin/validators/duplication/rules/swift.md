---
name: swift
description: Swift-specific duplication carve-outs — dispatch-forced override shims and conformance boilerplate
---

# Swift Duplication Carve-outs

Applies only to Swift files in scope. If no file in scope is Swift, this rule
contributes nothing.

Swift's dispatch rules force certain one-liners to repeat per class. These are
not duplication — do not flag them:

- **Identical one-line `override`s that forward into shared code** (a call to
  `super` and/or an already-extracted shared helper, typically a
  protocol-extension method). The language prevents hoisting them, all four
  escape routes are closed:
  - a protocol extension cannot call `super`;
  - `override`s cannot be declared in extensions;
  - classes with different superclasses cannot share a common base to host the
    override;
  - a member "extracted" into a protocol extension never enters class dynamic
    dispatch — the override silently stops being called, changing behavior.

  If the shared logic already lives in one place (e.g. a protocol extension)
  and only the per-class forwarding override repeats, the duplication is
  resolved. Demanding further extraction asks for code Swift cannot express.

- **Trivial conformance stubs the compiler requires per type** — e.g. a
  one-line `description`, `id`, or `CodingKeys` declaration — where the body
  carries no logic that could drift.

The test: does the repeated block contain logic that could drift out of sync?
A forwarding line (`super.freeze(); adapted(x)`) cannot drift — the logic it
forwards to is already shared. Flag the copies only when actual logic repeats.
