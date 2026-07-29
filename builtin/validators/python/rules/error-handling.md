---
name: error-handling
description: Never retry on bare Exception, match retry to error semantics, custom hierarchies
---

# Python Error Handling

- **Never retry on bare `Exception`.** Retrying on `Exception` hides programming errors as transient failures. Retry logic must list specific exception types, for example `requests.Timeout` or `sqlalchemy.exc.OperationalError`. A blanket catch-and-retry is a blocker.
- **Match retry logic to the type of error.** A 404 error is not transient. A 503 error is transient. Retry logic must tell these apart.
- **Use custom exception hierarchies.** Exceptions must inherit from a project-specific base exception, not directly from `Exception`. A library that raises a bare `ValueError` or `RuntimeError` for a domain failure has poor design.
- **Avoid `hasattr()` for flow control.** Use explicit `try/except AttributeError`, or use `getattr(obj, 'attr', None)`.
