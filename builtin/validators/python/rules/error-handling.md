---
name: error-handling
description: Never retry on bare Exception, match retry to error semantics, custom hierarchies
severity: error
---

# Python Error Handling

- **Never retry on bare `Exception`.** Retrying on `Exception` masks programming errors as transient failures. Retry logic must enumerate specific exception types (`requests.Timeout`, `sqlalchemy.exc.OperationalError`). Blanket catch-and-retry is a blocker.
- **Match retry semantics to error semantics.** A 404 is not transient. A 503 is. Retry logic that doesn't discriminate is incorrect.
- **Custom exception hierarchies.** Exceptions inherit from a project-specific base exception, not directly from `Exception`. Libraries that raise bare `ValueError` or `RuntimeError` for domain failures are poorly designed.
- **Avoid `hasattr()` for flow control.** Use explicit `try/except AttributeError` or `getattr(obj, 'attr', None)`.
