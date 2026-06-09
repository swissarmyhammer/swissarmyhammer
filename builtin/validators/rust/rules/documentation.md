---
name: documentation
description: Doc comments on public items, examples use ?, document panics and safety
severity: warn
---

# Rust Documentation

- All public items have doc comments.
- Examples use `?`, not `.unwrap()`. Showing unwrap in examples teaches bad habits.
- Panics, errors, and safety requirements documented.
- `Debug` implemented for all public types with non-empty representation.
- Crate-level docs include examples showing common use cases.
