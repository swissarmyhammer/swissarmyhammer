# Rust Review Guidelines (dtolnay school)

Apply these when reviewing Rust code. These supplement the universal review layers.

## Error Handling

**Library code uses typed errors; application code uses `anyhow`.**

- Libraries: return typed error enums via `thiserror`. Never return `anyhow::Error` or `Box<dyn Error>` from public APIs — callers lose the ability to match on specific failures.
- Applications: use `anyhow::Result<T>`. Every `?` on I/O or external calls must have `.context("what we were doing")`. A bare "No such file or directory" without context is a blocker.
- `Display` messages on errors: lowercase, no trailing punctuation.
- `Error::source()` chains must exist for wrapped errors — don't flatten the chain.
- Panics are for bugs only — internal invariant violations. Never panic on expected failure modes (bad input, missing files, network errors).

## Type Safety

- **Newtypes for semantic distinctions.** Two parameters of the same primitive type with different meanings (e.g., `user_id: u64, order_id: u64`) must use newtypes. Zero runtime cost, compile-time safety.
- **No adjacent `bool` parameters.** `Widget::new(true, false)` is unreadable. Use enums: `Widget::new(Small, Round)`.
- **Builder pattern** for structs with 3+ optional fields. Method chaining should feel natural.
- **Sealed traits** for public traits not meant to be implemented downstream. Prevents semver hazards when adding methods.

## API Design

- **Accept generics, not concrete types.** `&str` not `&String`, `&[T]` not `&Vec<T>`, `impl IntoIterator` not `&Vec<T>`, `AsRef<Path>` not `&PathBuf`.
- **Expose intermediate results.** Don't discard useful data computed along the way — return it in error types or as part of the result.
- **No `get_` prefix on getters.** Use `field_name()`, `field_mut()`.
- **Conversion naming:** `as_` (free, borrow→borrow), `to_` (expensive, borrow→owned), `into_` (free, owned→owned). Flag `to_bytes()` that just reinterprets memory (should be `as_bytes()`).
- **Implement `From`, not `Into`.** The blanket impl gives you `Into` for free. Implementing `Into` directly prevents the blanket from applying.

## Trait Implementations

New public types must implement all applicable traits. Due to orphan rules, if you don't, downstream crates can't add them.

Check for: `Clone`, `Debug`, `Display`, `Default`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `From`/`TryFrom`, `AsRef`, `Send`/`Sync` (add compile-time assertions for pointer types).

- Collections: implement `FromIterator` and `Extend`.
- `serde`: `Serialize`/`Deserialize` behind an optional feature flag.
- A new public type missing obvious trait impls is a silent semver hazard.

## Future-Proofing

- **Private struct fields.** Public fields are a permanent commitment. Use getters/setters.
- **Trait bounds on `impl` blocks, not type definitions.** Bounds on structs restrict what code can be written against your type and duplicate what derive macros infer.
- **Optional features are additive.** Enabling a feature must never break code that worked without it.
- **`no_std` support:** use a `std` feature that enables std-dependent code, not a `no_std` feature. Default to `no_std` where possible.

## Documentation

- All public items have doc comments.
- Examples use `?`, not `.unwrap()`. Showing unwrap in examples teaches bad habits.
- Panics, errors, and safety requirements documented.
- `Debug` implemented for all public types with non-empty representation.
- Crate-level docs include examples showing common use cases.
