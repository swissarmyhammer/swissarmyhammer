---
name: api-design
description: Keep serialization separate from classes, decorators preserve signatures
---

# Python API Design

- **Keep serialization separate from classes.** Do not add `to_json()` methods to domain objects. Use `cattrs`, `msgspec`, or `functools.singledispatch` as a separate serialization layer.
- **Decorators must preserve function signatures.** `functools.wraps` alone is not enough. It preserves `__name__` and `__doc__`, but not the callable signature. Use the `wrapt` library or the `decorator` library instead. Check that decorated functions work with frameworks that inspect signatures, such as FastAPI or click.
