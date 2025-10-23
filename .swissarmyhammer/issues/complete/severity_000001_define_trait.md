# Step 1: Define Severity Trait in swissarmyhammer-common

**Refer to ideas/severity.md**

## Goal

Define the `Severity` trait in `swissarmyhammer-common/src/error.rs` to provide a standardized way for all error types to report their severity level.

## Context

This is the foundation step for implementing severity levels across all SwissArmyHammer error types. The ErrorSeverity enum already exists in swissarmyhammer-common - we need to add a trait that error types will implement.

## Tasks

### 1. Define the Severity Trait

Add the trait definition to `swissarmyhammer-common/src/error.rs` near the ErrorSeverity enum:

```rust
/// Trait for error types that have severity levels
///
/// All SwissArmyHammer error types should implement this trait to provide
/// consistent severity reporting across the codebase.
///
/// # Severity Levels
///
/// - **Critical**: System cannot continue, data loss possible, requires immediate attention
/// - **Error**: Operation failed but system can continue, no data loss  
/// - **Warning**: Potential issue but operation can proceed
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_common::{ErrorSeverity, Severity};
///
/// enum MyError {
///     CriticalFailure,
///     NotFound,
///     Deprecated,
/// }
///
/// impl Severity for MyError {
///     fn severity(&self) -> ErrorSeverity {
///         match self {
///             MyError::CriticalFailure => ErrorSeverity::Critical,
///             MyError::NotFound => ErrorSeverity::Error,
///             MyError::Deprecated => ErrorSeverity::Warning,
///         }
///     }
/// }
/// ```
pub trait Severity {
    /// Get the severity level of this error
    fn severity(&self) -> ErrorSeverity;
}
```

### 2. Update Documentation

Add module-level documentation explaining:
- Purpose of the Severity trait
- Guidelines for assigning severity levels
- Examples of each severity level

### 3. Make ErrorSeverity Public and Documented

Ensure ErrorSeverity is properly documented and exported:

```rust
/// Severity levels for error classification
///
/// These levels help categorize errors by their impact and urgency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Potential issue but operation can proceed
    /// Examples: empty files, deprecation notices
    Warning,
    
    /// Operation failed but system can continue
    /// Examples: file not found, invalid format
    Error,
    
    /// System cannot continue, requires immediate attention
    /// Examples: database corruption, workflow failures
    Critical,
}
```

### 4. Export from Crate Root

Ensure the trait is exported from `swissarmyhammer-common/src/lib.rs`:

```rust
pub use error::{ErrorSeverity, Severity};
```

## Acceptance Criteria

- [ ] Severity trait defined with comprehensive documentation
- [ ] ErrorSeverity enum has detailed docs for each variant
- [ ] Trait and enum are exported from crate root
- [ ] Code compiles: `cargo build -p swissarmyhammer-common`
- [ ] Documentation builds: `cargo doc -p swissarmyhammer-common --no-deps`

## Files to Modify

- `swissarmyhammer-common/src/error.rs` (add trait definition, improve docs)
- `swissarmyhammer-common/src/lib.rs` (ensure exports)

## Estimated Changes

~40 lines of code (trait + documentation)

## Next Step

Step 2: Implement Severity for SwissArmyHammerError



## Proposed Solution

Based on examining the current code:

1. **ErrorSeverity enum already exists** (lines 12-19 of error.rs) but:
   - Has `#[allow(dead_code)]` which should be removed
   - Lacks detailed documentation on each variant
   - Missing `Eq` derive (has PartialEq but not Eq)

2. **Implementation steps**:
   - Define the `Severity` trait with clear documentation
   - Enhance ErrorSeverity documentation with examples for each severity level
   - Add the `Eq` derive to ErrorSeverity
   - Remove `#[allow(dead_code)]` 
   - Export both Severity and ErrorSeverity from lib.rs (ErrorSeverity already exported via `pub use error::*;`)
   - Write comprehensive tests for the trait

3. **Test strategy**:
   - Test that ErrorSeverity has proper ordering/equality
   - Test that a sample implementation of Severity works correctly
   - Test that the trait is accessible from the crate root
   - Verify documentation builds

This follows TDD: write tests first, then implement the trait to make tests pass.



## Implementation Notes

Successfully implemented the `Severity` trait following TDD principles:

### Changes Made

1. **Enhanced ErrorSeverity enum** (swissarmyhammer-common/src/error.rs:12-74):
   - Added comprehensive documentation with examples
   - Added detailed docs for each variant (Warning, Error, Critical)
   - Added `Eq` derive (was only `PartialEq`)
   - Removed `#[allow(dead_code)]` annotation

2. **Added Severity trait** (swissarmyhammer-common/src/error.rs:76-129):
   - Comprehensive trait documentation with guidelines
   - Clear examples showing how to implement the trait
   - Single method: `fn severity(&self) -> ErrorSeverity`

3. **Updated exports** (swissarmyhammer-common/src/lib.rs:62):
   - Exported both `Severity` and `ErrorSeverity` from crate root
   - Available via `use swissarmyhammer_common::{Severity, ErrorSeverity}`

4. **Added comprehensive tests**:
   - Unit tests in error.rs for ErrorSeverity equality
   - Unit tests in error.rs for Severity trait implementation
   - Integration tests in tests/exports.rs verifying crate exports

### Test Results

- All 196 tests pass in swissarmyhammer-common
- Entire workspace builds successfully
- Documentation builds without errors

### Files Modified

- `swissarmyhammer-common/src/error.rs` - Added trait and enhanced docs
- `swissarmyhammer-common/src/lib.rs` - Added exports
- `swissarmyhammer-common/tests/exports.rs` - New integration test file

### Acceptance Criteria Status

- ✅ Severity trait defined with comprehensive documentation
- ✅ ErrorSeverity enum has detailed docs for each variant
- ✅ Trait and enum are exported from crate root
- ✅ Code compiles: `cargo build -p swissarmyhammer-common`
- ✅ Documentation builds: `cargo doc -p swissarmyhammer-common --no-deps`
