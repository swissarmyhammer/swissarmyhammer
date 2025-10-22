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

## Proposed Solution

Based on my analysis of the code, here's my implementation plan:

### Current State Analysis
1. **ValidationError** (schema_validation.rs:12-85):
   - Already has a `severity()` method at lines 74-84
   - Already imports `ErrorSeverity` from swissarmyhammer_common (line 9)
   - Has test coverage at lines 700, 1121-1123
   - The method assigns severity levels to all 7 error variants

2. **ConversionError** (schema_conversion.rs:17-49):
   - Does NOT have a severity() method yet
   - Needs trait implementation
   - Has 6 error variants that need severity assignments

### Implementation Steps

#### Step 1: Refactor ValidationError
- Add `use swissarmyhammer_common::Severity;` import (line 10)
- Replace the existing `severity()` method with a trait implementation block
- Keep the exact same match logic and severity assignments
- Remove the `#[allow(dead_code)]` attribute from the method (line 73)

#### Step 2: Implement Severity for ConversionError  
- Add imports at top of schema_conversion.rs:
  ```rust
  use swissarmyhammer_common::{ErrorSeverity, Severity};
  ```
- Implement Severity trait with these assignments:
  - `MissingRequired`: ErrorSeverity::Error (user error)
  - `InvalidType`: ErrorSeverity::Error (user error)
  - `SchemaValidation`: ErrorSeverity::Critical (tool configuration issue)
  - `ParseError`: ErrorSeverity::Error (user input error)
  - `UnsupportedSchemaType`: ErrorSeverity::Error (tool compatibility issue)
  - `ValidationError`: ErrorSeverity from wrapped error (delegate)

#### Step 3: Add Tests
- ValidationError tests should continue to pass unchanged
- Add new test in schema_conversion.rs for ConversionError severity levels
- Test all 6 variants to ensure correct severity assignment

### Rationale for Severity Levels

**ValidationError** (keeping existing logic):
- Critical: InvalidSchema (blocks all usage)
- Error: UnsupportedSchemaType, MissingSchemaField, ConversionFailed, InvalidProperty, ConflictingDefinitions
- Warning: InvalidParameterName

**ConversionError** (new assignments):
- Critical: SchemaValidation (fundamental tool configuration problem)
- Error: MissingRequired, InvalidType, ParseError, UnsupportedSchemaType (all prevent execution)
- For ValidationError variant: delegate to the wrapped error's severity

### Test Strategy
1. Verify existing ValidationError tests pass (TDD step 1: tests already exist)
2. Add comprehensive ConversionError severity test
3. Run `cargo nextest run -p swissarmyhammer-cli`
4. Run `cargo clippy -p swissarmyhammer-cli`

This approach maintains backward compatibility while adding the trait-based interface.
