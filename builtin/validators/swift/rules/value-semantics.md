---
name: value-semantics
description: struct/enum over class, final classes, COW uniqueness, protocol extensions over base classes
---

# Swift Value Semantics

- **Default to `struct`/`enum`; use `class` only for genuine identity, reference semantics, or Obj-C interop.** A new model, config, container, or command with no identity/interop reason should be a value type. DON'T: `class UserProfile { var name: String }`. DO: `struct UserProfile { var name: String }`.
- **Don't reach for `class` just to mutate in place** — a `struct` with `mutating func` is the idiom. DO: `struct Repeat: ParsableCommand { mutating func run() throws { … } }`.
- **Mark classes not designed for subclassing `final`.** A non-`final` `class` that is never subclassed and isn't a deliberate extension point should be `final`.
- **Copy-on-write types check uniqueness before writing shared storage.** A `mutating` method that writes through a reference-typed buffer must call an `isKnownUniquelyReferenced`/`ensureUnique` guard first, and keep that buffer `internal` so the value-semantic surface stays pure.
- **Share behavior through protocol extensions and generic constraints, not base classes.** DO: `extension Collection where Element: Comparable { … }`. DON'T: a base `class` others subclass only to inherit helpers.
