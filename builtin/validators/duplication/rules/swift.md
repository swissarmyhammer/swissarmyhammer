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

- **Generic dispatch over a fixed set of distinct types** — a generic function
  requiring a concrete type parameter (e.g. `func register<N: SomeProtocol>(_:
  N.Type, ...)`), called once per type in a small, closed set, where each
  call's body differs because the underlying types genuinely differ (distinct
  associated types, distinct payload/parameter shapes) rather than from
  copy-paste. Swift resolves generic type parameters at compile time
  (monomorphization) — there is no way to iterate the set of types and make
  one call, so one call site per type is the only shape the language allows.
  Do not flag this unless a further shared abstraction would strictly reduce
  the code (not just relocate it) and preserve locality — moving each call
  into its own per-type conformance/extension so a reader must jump between
  N files instead of reading one table is worse, not better, and is not a fix.

The test: does the repeated block contain logic that could drift out of sync?
A forwarding line (`super.freeze(); adapted(x)`) cannot drift — the logic it
forwards to is already shared. Flag the copies only when actual logic repeats.
