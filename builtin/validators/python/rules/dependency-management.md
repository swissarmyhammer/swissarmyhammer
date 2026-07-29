---
name: dependency-management
description: Pin transitive deps in apps, minimum constraints in libraries, treat updates as breaking
---

# Python Dependency Management

- **Pin all transitive dependencies in applications.** Use lockfiles such as `uv lock`, `poetry.lock`, or `pip freeze`. Do not trust semantic versioning as a security posture.
- **Specify minimum version constraints in libraries.** Do not use exact pins. A library that pins `requests==2.31.0` creates conflicts for users.
- **Treat every update as a possible breaking change.** Test coverage is the only reliable protection. Version schemes are not reliable protection.
