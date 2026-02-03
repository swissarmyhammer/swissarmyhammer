---
name: rust-coding
description: Validates Rust code follows language patterns and conventions
severity: warn
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "**/*.rs"
tags:
  - rust
  - patterns
  - conventions
timeout: 30
---

# Rust Language Patterns and Conventions

## Core Patterns

### Error Handling
- Use `Result<T, E>` for recoverable errors
- Use `anyhow::Result<T>` for application-level errors where specific error types aren't needed
- Use `anyhow::Context` to add contextual information to errors
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
- Use ULID instead of UUID for unique identifiers
- Use enums for state machines and variant types
- Implement `Display` and `Debug` for public types
- Use `#[derive(Clone, Debug, PartialEq, Eq)]` as standard derives

### Function Design
- Avoid primitive types in function signatures, prefer domain-specific types
- Design functions that take single struct parameters over many individual parameters
  - Having a long list of primitive parameters is fragile - every new parameter requires updating all call sites
- Use builder pattern for complex object construction
  - use a popular macro like `derive_builder` to reduce boilerplate
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
- NEVER `println` or `eprintln` in unit tests

## Validation Patterns

### Input Validation
- Validate at system boundaries (API endpoints, file parsing)
- Use newtype patterns for validated data
- Implement `TryFrom` for conversion with validation
- Return descriptive validation errors

### Business Rule Validation
- Encode business rules in the type system when possible
- Use builder patterns with validation steps
- Validate complete objects, not individual fields
- Separate syntax validation from semantic validation

## Recovery Patterns

### Transaction Safety
- Use database transactions for multi-step operations
- Implement compensation actions for distributed transactions
- Store operation state for crash recovery
- Use write-ahead logs for durability

### Resource Cleanup
- Always use RAII patterns with `Drop` trait
- Implement cleanup in `Drop` even if explicit cleanup exists
- Use `scopeguard` crate for complex cleanup scenarios
- Never assume destructors will run (they might not in panics)
