---
name: api-design
description: Accept generics, expose intermediate results, conversion naming, From over Into
severity: warn
---

# Rust API Design

- **Accept generics, not concrete types.** `&str` not `&String`, `&[T]` not `&Vec<T>`, `impl IntoIterator` not `&Vec<T>`, `AsRef<Path>` not `&PathBuf`.
- **Expose intermediate results.** Don't discard useful data computed along the way — return it in error types or as part of the result.
- **No `get_` prefix on getters.** Use `field_name()`, `field_mut()`.
- **Conversion naming:** `as_` (free, borrow→borrow), `to_` (expensive, borrow→owned), `into_` (free, owned→owned). Flag `to_bytes()` that just reinterprets memory (should be `as_bytes()`).
- **Implement `From`, not `Into`.** The blanket impl gives you `Into` for free. Implementing `Into` directly prevents the blanket from applying.
