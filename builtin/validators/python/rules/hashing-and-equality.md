---
name: hashing-and-equality
description: Immutable objects with __eq__ must implement __hash__, never hash mutable attributes
severity: warn
---

# Python Hashing and Equality

- **Immutable objects with `__eq__` must implement `__hash__`.** Python 3 sets `__hash__ = None` when `__eq__` is defined, making objects unhashable.
- **Never hash mutable attributes.** A hash must be stable over the object's lifetime. Hashing a list or dict field produces silent bugs in sets and dicts.
