---
name: domain-separation
description: ORM classes are not domain models, no leaking impl types, facade third-party deps
severity: warn
---

# Python Domain Separation

- **ORM classes are not domain models.** Flag ORM calls scattered through view functions. Domain objects should be plain Python classes testable without a database.
- **Public APIs must not leak implementation types.** Functions should accept and return types the caller can construct without importing internal details.
- **Facade third-party dependencies.** Every external system (HTTP APIs, databases, queues) should be accessed through a wrapper you own. This enables mocking, isolates change, and simplifies testing.
