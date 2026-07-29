---
name: domain-separation
description: ORM classes are not domain models, no leaking impl types, facade third-party deps
---

# Python Domain Separation

- **Do not use ORM classes as domain models.** Flag ORM calls scattered through view functions. Domain objects must be plain Python classes. You must be able to test them without a database.
- **Public APIs must not leak implementation types.** Functions must accept and return types the caller can build without importing internal details.
- **Wrap third-party dependencies in a facade.** Access every external system, such as an HTTP API, a database, or a queue, through a wrapper you own. This wrapper enables mocking. It isolates change. It simplifies testing.
