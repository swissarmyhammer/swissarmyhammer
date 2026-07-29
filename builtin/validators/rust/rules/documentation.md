---
name: documentation
description: Add doc comments to public items, use ? in examples, and document panics and safety
---

# Rust Documentation

- All public items must have doc comments.
- Examples must use `?`, not `.unwrap()`. An example with `.unwrap()` teaches a bad habit.
- Document panics, errors, and safety requirements.
- Implement `Debug` for all public types with a non-empty representation.
- Crate-level docs must include examples that show common use cases.
