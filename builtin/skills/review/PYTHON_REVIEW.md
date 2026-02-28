# Python Review Guidelines (Hynek Schlawack school)

Apply these when reviewing Python code. These supplement the universal review layers.

## Class Design

- **Prefer `attrs.define` (or `dataclasses.dataclass`) over manual `__init__` boilerplate.** Hand-written `__init__` + `__repr__` + `__eq__` for data-holding classes is a red flag.
- **Make illegal states unrepresentable.** An `Optional[str]` field where `None` means "not initialized" is a design smell. Split into separate types or use a factory.
- **Composition over inheritance.** If `class B(A)` exists to reuse A's methods (not to specialize A's type), prefer wrapping or extracting shared logic. Use `typing.Protocol` for interface contracts, not abstract base classes with implementation.
- **Avoid subclass explosion.** If customization requires subclassing, prefer passing callables or configuration objects instead. Hierarchies deeper than two levels are a warning sign.

## Domain Separation

- **ORM classes are not domain models.** Flag ORM calls scattered through view functions. Domain objects should be plain Python classes testable without a database.
- **Public APIs must not leak implementation types.** Functions should accept and return types the caller can construct without importing internal details.
- **Facade third-party dependencies.** Every external system (HTTP APIs, databases, queues) should be accessed through a wrapper you own. This enables mocking, isolates change, and simplifies testing.

## Testing

- **Use the `src/` layout.** Tests must run against the installed package, not the source directory. Flat layouts hide packaging bugs.
- **Test docstrings are mandatory.** Every test explains _why_ it exists, not just what it does. "Empty strings are invalid identifiers and should be rejected at the boundary, not silently produce None downstream."
- **Don't mock what you don't own.** If a test mocks `boto3.client()` or `requests.get()` directly, flag it. Mock an owned facade wrapping the dependency instead.
- **Regression tests reference bug tracker issues.** Information that doesn't fit in the test name belongs in the docstring.

## Error Handling

- **Never retry on bare `Exception`.** Retrying on `Exception` masks programming errors as transient failures. Retry logic must enumerate specific exception types (`requests.Timeout`, `sqlalchemy.exc.OperationalError`). Blanket catch-and-retry is a blocker.
- **Match retry semantics to error semantics.** A 404 is not transient. A 503 is. Retry logic that doesn't discriminate is incorrect.
- **Custom exception hierarchies.** Exceptions inherit from a project-specific base exception, not directly from `Exception`. Libraries that raise bare `ValueError` or `RuntimeError` for domain failures are poorly designed.
- **Avoid `hasattr()` for flow control.** Use explicit `try/except AttributeError` or `getattr(obj, 'attr', None)`.

## Logging

- **Structured logging only.** `logger.info(f"Order {order_id} processed")` cannot be indexed or queried. Use `structlog` or equivalent: `logger.info("order.processed", order_id=order_id)`.
- **Log to stdout.** Let infrastructure (systemd, Docker, Kubernetes) handle routing. Applications should not configure log files or rotation.
- **JSON in production, pretty-print in development.** A hard-coded log format is a finding.

## Dependency Management

- **Applications: pin all transitive dependencies** in lockfiles (`uv lock`, `poetry.lock`, `pip freeze`). "Trust semver" is not a security posture.
- **Libraries: specify minimum version constraints**, not exact pins. A library pinning `requests==2.31.0` creates conflicts for users.
- **Treat every update as potentially breaking.** The only reliable protection is test coverage, not version schemes.

## API Design

- **Keep serialization separate from classes.** No `to_json()` methods on domain objects. Use `cattrs`, `msgspec`, or `functools.singledispatch` as a separate serialization layer.
- **Decorators must preserve function signatures.** `functools.wraps` alone is insufficient — it preserves `__name__` and `__doc__` but not the callable signature. Use `wrapt` or `decorator` library. Verify decorated functions work with frameworks that inspect signatures (FastAPI, click).

## Hashing and Equality

- **Immutable objects with `__eq__` must implement `__hash__`.** Python 3 sets `__hash__ = None` when `__eq__` is defined, making objects unhashable.
- **Never hash mutable attributes.** A hash must be stable over the object's lifetime. Hashing a list or dict field produces silent bugs in sets and dicts.
