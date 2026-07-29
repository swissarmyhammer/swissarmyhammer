---
name: api-design
description: Accept generics, expose intermediate results, name conversions clearly, and implement From over Into
---

# Rust API Design

- **Accept generic types, not concrete types.** Use `&str`, not `&String`. Use `&[T]`, not `&Vec<T>`. Use `impl IntoIterator`, not `&Vec<T>`. Use `AsRef<Path>`, not `&PathBuf`.
- **Expose intermediate results.** Do not discard useful data that you compute along the way. Return this data in error types, or as part of the result.
- **Do not use a `get_` prefix on getters.** Use `field_name()` or `field_mut()`.
- **Name conversion methods by cost and ownership.** Use `as_` for a free borrow-to-borrow conversion. Use `to_` for an expensive borrow-to-owned conversion. Use `into_` for a free owned-to-owned conversion. Flag a `to_bytes()` method that only reinterprets memory; rename it to `as_bytes()`.
- **Implement `From`, not `Into`.** The blanket implementation gives you `Into` for free. A direct implementation of `Into` blocks the blanket implementation from applying.
