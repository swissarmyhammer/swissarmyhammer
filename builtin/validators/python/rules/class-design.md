---
name: class-design
description: attrs/dataclasses over boilerplate, illegal states unrepresentable, composition
severity: warn
---

# Python Class Design

- **Prefer `attrs.define` (or `dataclasses.dataclass`) over manual `__init__` boilerplate.** Hand-written `__init__` + `__repr__` + `__eq__` for data-holding classes is a red flag.
- **Make illegal states unrepresentable.** An `Optional[str]` field where `None` means "not initialized" is a design smell. Split into separate types or use a factory.
- **Composition over inheritance.** If `class B(A)` exists to reuse A's methods (not to specialize A's type), prefer wrapping or extracting shared logic. Use `typing.Protocol` for interface contracts, not abstract base classes with implementation.
- **Avoid subclass explosion.** If customization requires subclassing, prefer passing callables or configuration objects instead. Hierarchies deeper than two levels are a warning sign.
