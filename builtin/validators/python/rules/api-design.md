---
name: api-design
description: Keep serialization separate from classes, decorators preserve signatures
severity: warn
---

# Python API Design

- **Keep serialization separate from classes.** No `to_json()` methods on domain objects. Use `cattrs`, `msgspec`, or `functools.singledispatch` as a separate serialization layer.
- **Decorators must preserve function signatures.** `functools.wraps` alone is insufficient — it preserves `__name__` and `__doc__` but not the callable signature. Use `wrapt` or `decorator` library. Verify decorated functions work with frameworks that inspect signatures (FastAPI, click).
