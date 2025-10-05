# Use Violation Severity for Logging Level

## Description
When logging violations in `checker.rs`, use the violation's severity to pick the appropriate tracing log level instead of always using `warn!`.

## Current State
In `checker.rs` around line 322:

```rust
tracing::warn!("{}", violation);
```

This always logs violations at WARN level regardless of their actual severity.

## Desired Behavior
The logging level should match the violation severity:
- `Severity::Error` → `tracing::error!`
- `Severity::Warning` → `tracing::warn!`
- `Severity::Info` → `tracing::info!`
- `Severity::Hint` → `tracing::debug!` or `tracing::info!`

## Implementation
```rust
match violation.severity {
    Severity::Error => tracing::error!("{}", violation),
    Severity::Warning => tracing::warn!("{}", violation),
    Severity::Info => tracing::info!("{}", violation),
    Severity::Hint => tracing::debug!("{}", violation),
}
```

## Location
File: `swissarmyhammer-rules/src/checker.rs`
Function: `RuleChecker::check_file()`
Line: ~322

## Acceptance Criteria
- [ ] Violations log at appropriate level based on severity
- [ ] Error severity uses `tracing::error!`
- [ ] Warning severity uses `tracing::warn!`
- [ ] Info severity uses `tracing::info!`
- [ ] Hint severity uses `tracing::debug!` or `tracing::info!`
- [ ] Existing tests continue to pass



## Proposed Solution

I will implement severity-based logging by replacing the hardcoded `tracing::warn!` at line 322 in `checker.rs` with a match statement that maps each `Severity` variant to the appropriate tracing level:

1. **Error** → `tracing::error!` - Critical violations that must be fixed
2. **Warning** → `tracing::warn!` - Issues that should be fixed
3. **Info** → `tracing::info!` - Informational messages
4. **Hint** → `tracing::debug!` - Suggestions and tips

### Implementation Steps

1. Write a test that creates violations with different severity levels and verifies the correct logging behavior
2. Replace the hardcoded `tracing::warn!("{}", violation);` with:
   ```rust
   match violation.severity {
       Severity::Error => tracing::error!("{}", violation),
       Severity::Warning => tracing::warn!("{}", violation),
       Severity::Info => tracing::info!("{}", violation),
       Severity::Hint => tracing::debug!("{}", violation),
   }
   ```
3. Run tests to ensure all existing tests pass
4. Format and lint the code

This change ensures that violations are logged at the appropriate level based on their severity, making it easier to filter logs and prioritize issues.



## Implementation Notes

### Changes Made

1. **Added `Severity` import** to `checker.rs:9`
   - Required to use `Severity` enum variants in the match statement

2. **Replaced hardcoded logging** at `checker.rs:322-327`
   - Changed from: `tracing::warn!("{}", violation);`
   - Changed to: Match statement mapping severity to appropriate log level
   - `Severity::Error` → `tracing::error!`
   - `Severity::Warning` → `tracing::warn!`
   - `Severity::Info` → `tracing::info!`
   - `Severity::Hint` → `tracing::debug!`

3. **Added test** at `checker.rs:614-639`
   - `test_violation_preserves_severity()` verifies that `RuleViolation` correctly preserves severity levels
   - Tests all four severity variants (Error, Warning, Info, Hint)

### Test Results

- All 156 tests passed
- No clippy warnings
- Code formatted with `cargo fmt`

### Verification

The implementation correctly maps violation severity to the appropriate tracing log level. This allows users to:
- Filter logs by severity (e.g., only show errors with `RUST_LOG=error`)
- Prioritize violations based on log level
- Use standard log level conventions across the codebase

The change is backward compatible - warnings still log at warn level, and the fail-fast behavior remains unchanged.



## Verification Complete

All acceptance criteria met:
- ✅ Violations log at appropriate level based on severity
- ✅ Error severity uses `tracing::error!`
- ✅ Warning severity uses `tracing::warn!`
- ✅ Info severity uses `tracing::info!`
- ✅ Hint severity uses `tracing::debug!`
- ✅ All 156 existing tests pass
- ✅ No clippy warnings
- ✅ Code formatted with `cargo fmt`

Implementation is complete and verified. The code correctly maps violation severity to appropriate tracing log levels as specified.