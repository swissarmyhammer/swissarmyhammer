---
name: value-semantics
description: struct/enum over class, final classes, COW uniqueness, protocol extensions over base classes
---

# Swift Value Semantics

- **Default to `struct`/`enum`. Use `class` only for genuine identity, reference semantics, or Obj-C interop.** A new model, config, container, or command must be a value type unless it has an identity or interop reason. DON'T: `class UserProfile { var name: String }`. DO: `struct UserProfile { var name: String }`.
- **Do not use `class` only to mutate in place.** A `struct` with `mutating func` is the idiomatic choice. DO: `struct Repeat: ParsableCommand { mutating func run() throws { … } }`.
- **Mark a class `final` when you do not design it for subclassing.** A non-`final` `class` that is never subclassed and is not a deliberate extension point must be `final`.
- **Check uniqueness before a copy-on-write type writes to shared storage.** A `mutating` method that writes through a reference-typed buffer must call an `isKnownUniquelyReferenced`/`ensureUnique` guard first, and must keep that buffer `internal` so the value-semantic surface stays pure.
- **Share behavior through protocol extensions and generic constraints, not base classes.** DO: `extension Collection where Element: Comparable { … }`. DON'T: a base `class` others subclass only to inherit helpers.
