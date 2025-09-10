# Rust Language Patterns and Conventions

## Core Patterns

### Error Handling
- Use `Result<T, E>` for recoverable errors
- Use `anyhow::Result<T>` for application-level errors where specific error types aren't needed
- Use `thiserror` for library-level errors that need specific types
- Always propagate errors with `?` operator when possible
- Never use `unwrap()` or `expect()` in production code except for truly impossible cases

### Memory Management
- Prefer owned types (`String`, `Vec<T>`) over borrowed types in struct fields
- Use `Arc<T>` for shared immutable data across threads
- Use `Arc<Mutex<T>>` or `Arc<RwLock<T>>` for shared mutable data
- Avoid `Rc<T>` and `RefCell<T>` in favor of clearer ownership patterns

### Type System
- Create newtype wrappers for domain-specific identifiers
- Use enums for state machines and variant types
- Implement `Display` and `Debug` for public types
- Use `#[derive(Clone, Debug, PartialEq, Eq)]` as standard derives

### Function Design
- Prefer functions that take single struct parameters over many individual parameters
- Use builder pattern for complex object construction
- Return `impl Trait` for complex return types when possible
- Use `&self` methods on types that represent resources or state

### Module Organization
- One public type per file when possible
- Use `mod.rs` for module initialization and re-exports
- Keep `lib.rs` minimal - mainly for re-exports and top-level docs
- Organize by domain concepts, not by technical layers

### Async Patterns
- Use `tokio` as the async runtime
- Prefer `async fn` over returning `impl Future`
- Use `Arc<tokio::sync::Mutex<T>>` for shared async state
- Always handle cancellation gracefully with `tokio::select!`

### Testing
- Use `#[cfg(test)]` modules in the same file as the code under test
- Create integration tests in `tests/` directory
- Use `proptest` for property-based testing
- Mock external dependencies, never internal ones

### Serialization
- Use `serde` with `serde_derive` for JSON/YAML serialization
- Use `#[serde(rename_all = "snake_case")]` for consistent naming
- Implement custom serialization only when necessary

### Logging
- Use `tracing` crate for structured logging
- Use `tracing::info!`, `tracing::warn!`, etc. instead of `println!`
- Add context with `tracing::instrument` on functions
- Use `tracing::error!` for errors, not `eprintln!`