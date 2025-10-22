# Step 4: Implement Severity for CLI Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for CLI error types in swissarmyhammer-cli crate, refactoring existing severity() methods to use the trait.

## Context

The CLI crate has ValidationError and ConversionError types. ValidationError ALREADY has a severity() method that returns ErrorSeverity - we need to refactor it to implement the trait instead.

## Tasks

### 1. Ensure swissarmyhammer-common Dependency

Verify `swissarmyhammer-cli/Cargo.toml` depends on swissarmyhammer-common (it already does).

### 2. Refactor ValidationError Severity Method

In `swissarmyhammer-cli/src/schema_validation.rs`:

**Before** (lines 74-83):
```rust
pub fn severity(&self) -> ErrorSeverity {
    match self {
        ValidationError::UnsupportedSchemaType { .. } => ErrorSeverity::Error,
        // ... other variants
    }
}
```

**After**:
```rust
impl Severity for ValidationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ValidationError::UnsupportedSchemaType { .. } => ErrorSeverity::Error,
            ValidationError::InvalidSchema { .. } => ErrorSeverity::Critical,
            ValidationError::MissingSchemaField { .. } => ErrorSeverity::Error,
            ValidationError::ConversionFailed { .. } => ErrorSeverity::Error,
            ValidationError::InvalidParameterName { .. } => ErrorSeverity::Warning,
            ValidationError::InvalidProperty { .. } => ErrorSeverity::Error,
            ValidationError::ConflictingDefinitions { .. } => ErrorSeverity::Error,
        }
    }
}
```

### 3. Add Use Statement

At the top of `swissarmyhammer-cli/src/schema_validation.rs`:

```rust
use swissarmyhammer_common::Severity;
```

### 4. Implement Severity for ConversionError

In `swissarmyhammer-cli/src/schema_conversion.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ConversionError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ConversionError::InvalidSchema { .. } => ErrorSeverity::Critical,
            ConversionError::UnsupportedType { .. } => ErrorSeverity::Error,
            ConversionError::MissingField { .. } => ErrorSeverity::Error,
            ConversionError::ConversionFailed { .. } => ErrorSeverity::Error,
            // Add other variants with appropriate severity
        }
    }
}
```

### 5. Update Tests

The existing tests in `swissarmyhammer-cli/src/schema_validation.rs` at lines 700, 1121-1123 should still work because the method signature hasn't changed - it's just now coming from a trait.

Verify tests still pass and update if needed.

### 6. Add Tests for ConversionError

Add severity tests for ConversionError in `swissarmyhammer-cli/src/schema_conversion.rs`.

## Acceptance Criteria

- [ ] ValidationError implements Severity trait
- [ ] ConversionError implements Severity trait  
- [ ] Existing tests still pass
- [ ] New tests added for ConversionError severity
- [ ] Tests pass: `cargo test -p swissarmyhammer-cli`
- [ ] Code compiles: `cargo build -p swissarmyhammer-cli`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-cli`

## Files to Modify

- `swissarmyhammer-cli/src/schema_validation.rs` (refactor to trait impl)
- `swissarmyhammer-cli/src/schema_conversion.rs` (add trait impl + tests)

## Estimated Changes

~60 lines of code (2 implementations + tests)

## Next Step

Step 5: Implement Severity for config errors
