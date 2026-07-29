---
name: class-design
description: attrs/dataclasses over boilerplate, illegal states unrepresentable, composition
---

# Python Class Design

- **Use `attrs.define` or `dataclasses.dataclass` instead of manual `__init__` boilerplate.** A hand-written `__init__`, `__repr__`, and `__eq__` for a data-holding class is a warning sign.
- **Make illegal states unrepresentable.** An `Optional[str]` field where `None` means "not initialized" is a design flaw. Split the field into separate types, or use a factory.
- **Use composition instead of inheritance.** Suppose `class B(A)` exists only to reuse methods from A, not to specialize the type of A. In this case, wrap A or extract the shared logic instead. Use `typing.Protocol` for interface contracts. Do not use abstract base classes with implementation.
- **Avoid a deep chain of subclasses.** If customization needs subclassing, pass callables or configuration objects instead. A hierarchy deeper than two levels is a warning sign.
