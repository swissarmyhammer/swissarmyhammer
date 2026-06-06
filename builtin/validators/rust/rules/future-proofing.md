---
name: future-proofing
description: Private fields, bounds on impl blocks, additive features, no_std support
severity: warn
---

# Rust Future-Proofing

- **Private struct fields.** Public fields are a permanent commitment. Use getters/setters.
- **Trait bounds on `impl` blocks, not type definitions.** Bounds on structs restrict what code can be written against your type and duplicate what derive macros infer.
- **Optional features are additive.** Enabling a feature must never break code that worked without it.
- **`no_std` support:** use a `std` feature that enables std-dependent code, not a `no_std` feature. Default to `no_std` where possible.
