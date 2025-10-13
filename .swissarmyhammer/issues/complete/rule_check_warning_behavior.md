# Do not early exit on warnings in rule check

## Problem
Currently when running `sah rule check`, the command exits early on warnings. This prevents checking all rules when a warning is encountered.

## Desired Behavior
- Warnings should be logged but should NOT cause early exit
- Only errors should trigger early exit
- This allows all rules to be evaluated even when warnings are present

## Implementation Notes
- Review the rule check command implementation
- Separate warning handling from error handling
- Ensure warnings are still properly logged but don't stop execution
- Only exit early when encountering actual errors


## Proposed Solution

### Root Cause
In `swissarmyhammer-rules/src/checker.rs`, the `check_file` method returns `Err(RuleError::Violation(violation))` for **all** violations regardless of severity. The `check_all` method uses the `?` operator which propagates any error immediately, causing early exit even for warnings.

### Implementation Plan

1. **Modify check_file signature and return type**
   - Change `check_file` to return a Result that distinguishes between errors and warnings
   - Keep returning `Err()` for Error severity violations (to maintain fail-fast for errors)
   - Return `Ok(())` for Warning/Info/Hint violations (after logging them)

2. **Alternative approach (preferred):**
   - Modify `check_all` to collect warnings but continue execution
   - Only propagate errors immediately
   - At the end, if warnings were found, decide whether to exit with error code

3. **Testing strategy:**
   - Create test with a Warning-severity rule that should not stop execution
   - Create test with an Error-severity rule that should stop execution
   - Verify both behaviors work correctly

### Decision
Using approach 1 (simpler): Modify `check_file` to only return `Err()` for Error severity, and return `Ok(())` after logging for Warning/Info/Hint severities.

This ensures:
- Errors cause fail-fast behavior ✓
- Warnings are logged but don't stop execution ✓
- Info and Hint violations are also logged but don't stop execution ✓



## Implementation

### Changes Made

**File: `swissarmyhammer-rules/src/checker.rs`**

Modified the `check_file` method to differentiate behavior based on violation severity:

1. **Error severity**: Returns `Err()` to cause fail-fast behavior (existing behavior preserved)
2. **Warning/Info/Hint severity**: Returns `Ok()` after logging the violation, allowing execution to continue

### Key Code Change

In the violation handling section (around line 340-360), replaced:
```rust
Err(RuleError::Violation(violation).into())
```

With severity-based decision:
```rust
match violation.severity {
    Severity::Error => Err(RuleError::Violation(violation).into()),
    Severity::Warning | Severity::Info | Severity::Hint => {
        tracing::debug!(
            "Non-error violation logged, continuing execution: {} in {}",
            violation.rule_name,
            target_path.display()
        );
        Ok(())
    }
}
```

### Testing

**Added tests in `swissarmyhammer-rules/tests/checker_integration_test.rs`:**

1. `test_warning_does_not_stop_execution` - Verifies warnings are logged but don't stop checking subsequent files
2. `test_error_does_stop_execution` - Verifies errors still cause fail-fast behavior

**Test Results:**
- All 168 tests in swissarmyhammer-rules pass ✓
- All 1153 tests in swissarmyhammer-cli pass ✓

### Behavior Summary

**Before fix:**
- Any violation (Error, Warning, Info, Hint) caused immediate exit
- Only first file would be checked when violations present

**After fix:**
- Error violations cause immediate exit (fail-fast preserved) ✓
- Warning violations are logged but execution continues ✓
- Info violations are logged but execution continues ✓
- Hint violations are logged but execution continues ✓
- All files are checked unless Error severity violation is found ✓

### Cache Behavior

The caching behavior remains unchanged:
- Both Error and non-Error violations are cached
- Cached violations are replayed with appropriate logging
- Cache prevents redundant LLM calls for same file+rule combinations



## Code Review Improvements

### Issue Identified in Code Review

During code review, a critical bug was found: **cached violations were not applying severity-based behavior**. 

**Problem:** In `checker.rs:261-270`, cached violations always returned `Err()` regardless of severity, but fresh violations applied severity-based logic (Error = Err, Warning/Info/Hint = Ok). This inconsistency meant:
- Fresh warning violations: continued execution ✓
- Cached warning violations: stopped execution ✗

### Changes Made

#### 1. Fixed Cached Violation Behavior (`checker.rs:261-283`)

**Before:**
```rust
CachedResult::Violation { violation } => {
    // Log the violation with appropriate severity
    match violation.severity {
        Severity::Error => tracing::error!("{}", violation),
        Severity::Warning => tracing::warn!("{}", violation),
        Severity::Info => tracing::info!("{}", violation),
        Severity::Hint => tracing::debug!("{}", violation),
    }
    return Err(RuleError::Violation(violation).into());  // ❌ Wrong for warnings!
}
```

**After:**
```rust
CachedResult::Violation { violation } => {
    // Log the violation with appropriate severity
    match violation.severity {
        Severity::Error => tracing::error!("{}", violation),
        Severity::Warning => tracing::warn!("{}", violation),
        Severity::Info => tracing::info!("{}", violation),
        Severity::Hint => tracing::debug!("{}", violation),
    }

    // Apply same severity-based behavior as fresh evaluation
    match violation.severity {
        Severity::Error => return Err(RuleError::Violation(violation).into()),
        Severity::Warning | Severity::Info | Severity::Hint => {
            tracing::debug!(
                "Cached non-error violation logged, continuing execution: {} in {}",
                violation.rule_name,
                target_path.display()
            );
            return Ok(());  // ✓ Correct behavior!
        }
    }
}
```

#### 2. Added Test for Cached Warning Behavior

**Test:** `test_cached_warning_does_not_stop_execution` in `checker_integration_test.rs`

This test verifies that cached warnings behave identically to fresh warnings:
1. First run: fresh evaluation creates cached warning → should continue execution
2. Second run: cached warning replayed → should also continue execution

#### 3. Added Tests for Mixed Severity Scenarios

**Test 1:** `test_mixed_severities_across_rules`
- Tests file with TODO checked by both Warning and Error severity rules
- Tests multiple Warning rules on the same file
- Verifies errors stop execution but warnings don't

**Test 2:** `test_mixed_severities_across_multiple_files`
- Tests warnings across multiple files with TODOs
- Verifies all files are checked despite warnings in multiple files

#### 4. Enhanced Cache Module Documentation

Added comprehensive explanation in `cache.rs:7-16` of why SHA-256 hashes are used:
- Git operations can reset file timestamps
- File system operations modify timestamps unpredictably
- Content-based hashing provides deterministic cache keys
- Rule template inclusion ensures cache invalidation on rule changes

### Test Results

**All tests pass:**
- 171 tests in swissarmyhammer-rules: ✓ PASSED
- Cargo clippy with `-D warnings`: ✓ NO WARNINGS
- Cargo fmt: ✓ FORMATTED

### Impact

**Before fixes:**
- Cached warnings stopped execution (bug)
- Test coverage gap for cache behavior
- No tests for mixed severity scenarios
- Cache strategy rationale not documented

**After fixes:**
- Cached warnings now behave correctly ✓
- Comprehensive test coverage for cache behavior ✓
- Edge cases for mixed severities covered ✓
- Clear documentation of design decisions ✓
