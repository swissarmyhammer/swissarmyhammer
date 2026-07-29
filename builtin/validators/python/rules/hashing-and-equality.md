---
name: hashing-and-equality
description: Immutable objects with __eq__ must implement __hash__, never hash mutable attributes
---

# Python Hashing and Equality

- **Immutable objects that define `__eq__` must implement `__hash__`.** Python 3 sets `__hash__ = None` when you define `__eq__`. This makes the object unhashable.
- **Never hash mutable attributes.** A hash must stay stable over the life of the object. Hashing a list field or a dict field causes silent bugs in sets and dicts.
