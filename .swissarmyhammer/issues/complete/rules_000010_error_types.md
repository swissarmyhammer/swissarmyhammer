# Define RuleViolation and RuleError Types

Refer to ideas/rules.md

## Goal

Define error types for the rules system, including RuleViolation for fail-fast behavior.

## Context

The rules system needs clear error types for different failure modes. RuleViolation is special - it's an error that represents a rule violation (used for fail-fast).

## Implementation

1. Create `src/error.rs` module
2. Define `RuleError` enum:
```rust
pub enum RuleError {
    LoadError(String),          // Can't load rule file
    ValidationError(String),    // Rule is invalid
    CheckError(String),         // Error during checking
    AgentError(String),         // LLM agent failed
    LanguageDetectionError(String),
    GlobExpansionError(String),
    Violation(RuleViolation),   // Rule violation found (for fail-fast)
}
```

3. Define `RuleViolation` struct:
```rust
pub struct RuleViolation {
    pub rule_name: String,
    pub file_path: PathBuf,
    pub severity: Severity,
    pub message: String,  // Full LLM response
}
```

4. Implement Display and std::error::Error traits
5. Add conversion methods

## Testing

- Unit tests for error creation
- Unit tests for error display

## Success Criteria

- [ ] RuleError enum defined
- [ ] RuleViolation struct defined
- [ ] Error traits implemented
- [ ] Unit tests passing



## Proposed Solution

Based on the issue specification and existing code structure, I will:

1. Create `src/error.rs` module with `RuleError` enum and `RuleViolation` struct
2. Define `RuleError` variants for different failure modes:
   - `LoadError` - Cannot load rule file
   - `ValidationError` - Rule is invalid
   - `CheckError` - Error during checking
   - `AgentError` - LLM agent failed
   - `LanguageDetectionError` - Cannot detect language
   - `GlobExpansionError` - Cannot expand glob pattern
   - `Violation` - Rule violation found (for fail-fast)
3. Define `RuleViolation` struct with rule name, file path, severity, and message
4. Implement `Display` and `std::error::Error` traits for proper error handling
5. Add conversion methods for ergonomic error handling
6. Write comprehensive unit tests

The error types will integrate with the existing `SwissArmyHammerError` used throughout the codebase while providing rule-specific error variants.


## Implementation Complete

Successfully implemented error types for the rules system:

### Created Files
- `swissarmyhammer-rules/src/error.rs` - New module containing error types

### Error Types Defined

**RuleViolation struct:**
- Fields: `rule_name`, `file_path`, `severity`, `message`
- Implements: `Debug`, `Clone`, `PartialEq`, `Eq`, `Display`
- Constructor: `new()` method for ergonomic creation
- Used for fail-fast behavior when violations are found

**RuleError enum with variants:**
- `LoadError(String)` - Cannot load rule file
- `ValidationError(String)` - Rule is invalid
- `CheckError(String)` - Error during checking
- `AgentError(String)` - LLM agent failed
- `LanguageDetectionError(String)` - Cannot detect language
- `GlobExpansionError(String)` - Cannot expand glob pattern
- `Violation(RuleViolation)` - Rule violation found (for fail-fast)

**Traits Implemented:**
- `Display` for both `RuleError` and `RuleViolation`
- `std::error::Error` for `RuleError`
- `From<RuleError>` for `SwissArmyHammerError`
- `From<SwissArmyHammerError>` for `RuleError`

### Integration
- Added `error` module to `lib.rs`
- Exported `RuleError` and `RuleViolation` as public types
- Used `SwissArmyHammerError::other()` for conversion (not `Generic`)

### Testing
- 19 comprehensive unit tests covering:
  - Error creation and construction
  - Display formatting for all variants
  - Trait implementations (Error, Display, Clone, PartialEq)
  - Conversion between RuleError and SwissArmyHammerError
  - Different severity levels
  - Equality comparisons
- All 111 tests passing in the rules crate

### Build Status
- ✅ Compilation successful
- ✅ All tests passing
- ✅ Code formatted with cargo fmt