---
name: dependency-management
description: Pin transitive deps in apps, minimum constraints in libraries, treat updates as breaking
severity: warn
---

# Python Dependency Management

- **Applications: pin all transitive dependencies** in lockfiles (`uv lock`, `poetry.lock`, `pip freeze`). "Trust semver" is not a security posture.
- **Libraries: specify minimum version constraints**, not exact pins. A library pinning `requests==2.31.0` creates conflicts for users.
- **Treat every update as potentially breaking.** The only reliable protection is test coverage, not version schemes.
