---
name: preferred-crates
description: Use the project's preferred crates for common functionality
---

# Preferred Crates

This project standardizes on specific crates for common functionality. Using the
preferred crate ensures consistency across the workspace and avoids duplicate
dependencies that solve the same problem differently.

## What to Check

When adding new dependencies or implementing functionality, verify the correct
crate is used:

### Identifiers
- **Use `ulid`** for unique identifiers, not `uuid`. ULIDs are sortable by
  creation time, encode to shorter strings, and are monotonically increasing
  within a millisecond.

### Builder Pattern
- **Use `derive_builder`** for builder-pattern construction. Don't hand-roll
  builder structs with `set_foo` / `build` methods when `derive_builder` can
  generate them from a struct definition.

### Property-Based Testing
- **Use `proptest`** for property-based and fuzz-like testing. Prefer `proptest`
  over `quickcheck` for its better shrinking, more expressive strategies, and
  `proptest!` macro syntax.

### Benchmarking
- **Use `criterion`** for microbenchmarks. Don't use the unstable `#[bench]`
  attribute or `test::Bencher`. Criterion provides statistical analysis,
  regression detection, and HTML reports.

### Table/Listing Output
- **Use `comfy-table`** for CLI tabular output (see `comfy-table-listing` rule
  for details).

### Error Handling
- **Use `anyhow`** for application-level error handling.
- **Use `thiserror`** for library-level typed errors.

### Serialization
- **Use `serde`** with `serde_derive` for all serialization/deserialization.
- Apply `#[serde(rename_all = "snake_case")]` on enums for consistent naming
  at serialization boundaries.

### Logging and Tracing
- **Use `tracing`** for structured logging and diagnostics. Not `log`, not
  `env_logger`, not `println!`.

### Async Runtime
- **Use `tokio`** as the async runtime. Not `async-std`, not `smol`.

### Temporary Files in Tests
- **Use `tempfile`** for temporary directories and files in tests.

## What Passes

- `use ulid::Ulid` for generating IDs
- `#[derive(Builder)]` from `derive_builder` on configuration structs
- `proptest! { ... }` test blocks
- `criterion_group!` and `criterion_main!` for benchmarks
- `anyhow::Result<T>` in binary/application code
- `#[derive(thiserror::Error)]` in library error types
- `tracing::info!` for status messages

## What Fails

- `use uuid::Uuid` for new identifiers (existing UUIDs from external systems are fine)
- Hand-written `impl FooBuilder { fn set_bar(...) }` when `derive_builder` would work
- `use quickcheck` when `proptest` is the project standard
- `#[bench] fn bench_foo(b: &mut Bencher)` instead of criterion
- `use log::info` instead of `tracing::info`
- `#[tokio::main]` mixed with `async-std` in the same workspace
- `env_logger::init()` instead of a tracing subscriber
