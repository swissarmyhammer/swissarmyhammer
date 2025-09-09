# Move create_test_rate_limiter to swissarmyhammer-common

## Problem
The `create_test_rate_limiter()` function is **duplicated across multiple files** when it should be centralized in `swissarmyhammer-common` as shared test infrastructure. This violates DRY principles and creates maintenance overhead.

## Evidence of Duplication

### **Multiple Implementations Found:**

#### **swissarmyhammer-tools/src/test_utils.rs:20**
```rust
pub fn create_test_rate_limiter() -> Arc<RateLimiter> {
```

#### **swissarmyhammer-tools/tests/file_tools_integration_tests.rs:27**
```rust
fn create_test_rate_limiter() -> Arc<RateLimiter> {
```

#### **swissarmyhammer-tools/tests/notify_integration_tests.rs:19** 
```rust
fn create_test_rate_limiter() -> Arc<RateLimiter> {
```

### **Usage Across Codebase:**
- `swissarmyhammer-tools/src/test_utils.rs:53` - Uses its own version
- `swissarmyhammer-tools/tests/file_tools_integration_tests.rs:125` - Uses its own version
- `swissarmyhammer-tools/tests/notify_integration_tests.rs:43` - Uses its own version

## Current Problems
- ❌ **Code Duplication**: Same function implemented multiple times
- ❌ **Maintenance Burden**: Changes must be made in multiple places
- ❌ **Inconsistency Risk**: Different implementations could diverge
- ❌ **Wrong Location**: Test utilities should be in common crate

## Proposed Solution
**Centralize `create_test_rate_limiter()` in `swissarmyhammer-common`** where it belongs as shared test infrastructure.

## Implementation Plan

### Phase 1: Add Function to swissarmyhammer-common
- [ ] Add `create_test_rate_limiter()` function to `swissarmyhammer-common/src/test_utils.rs`
- [ ] Or create `swissarmyhammer-common/src/rate_limiter.rs` with test utilities if it doesn't exist
- [ ] Ensure function has proper test configuration and dependencies
- [ ] Export function from `swissarmyhammer-common/src/lib.rs`

### Phase 2: Update swissarmyhammer-tools/src/test_utils.rs
- [ ] Remove local `create_test_rate_limiter()` implementation from line 20
- [ ] Add import to use common crate version:
  ```rust
  use swissarmyhammer_common::create_test_rate_limiter;
  // OR
  use swissarmyhammer_common::test_utils::create_test_rate_limiter;
  ```
- [ ] Update usage on line 53 to use imported version
- [ ] Verify test utilities still work

### Phase 3: Update file_tools_integration_tests.rs
- [ ] Remove local `create_test_rate_limiter()` implementation from line 27
- [ ] Add import to use common crate version:
  ```rust
  use swissarmyhammer_common::create_test_rate_limiter;
  ```
- [ ] Update usage on line 125 to use imported version
- [ ] Verify file tools tests still work

### Phase 4: Update notify_integration_tests.rs  
- [ ] Remove local `create_test_rate_limiter()` implementation from line 19
- [ ] Add import to use common crate version:
  ```rust
  use swissarmyhammer_common::create_test_rate_limiter;
  ```
- [ ] Update usage on line 43 to use imported version
- [ ] Verify notify tests still work

### Phase 5: Clean Up Any Other Duplications
- [ ] Search for any other duplicate test rate limiter implementations
- [ ] Update all to use centralized version from common crate
- [ ] Ensure consistent test rate limiter configuration across all tests

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify rate limiter test utilities work
- [ ] Test that rate limiting behavior is consistent across all tools
- [ ] Ensure no test functionality is lost

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

```bash
# Should return ZERO results (no local implementations):
rg "fn create_test_rate_limiter" swissarmyhammer-tools/

# Should find imports from common crate:
rg "use.*swissarmyhammer_common.*create_test_rate_limiter" swissarmyhammer-tools/

# Should find implementation in common crate:
rg "fn create_test_rate_limiter" swissarmyhammer-common/
```

## Expected Implementation Location

### **swissarmyhammer-common/src/test_utils.rs** (or rate_limiter.rs)
```rust
pub fn create_test_rate_limiter() -> Arc<RateLimiter> {
    Arc::new(RateLimiter::with_config(RateLimiterConfig {
        requests_per_minute: 1000, // High limit for tests
        burst_size: 100,
        // ... test-appropriate configuration
    }))
}
```

## Benefits
- **Eliminate Duplication**: Single implementation instead of 3+
- **Consistent Testing**: Same rate limiter configuration across all tests
- **Shared Infrastructure**: Test utilities in common crate where they belong
- **Easier Maintenance**: Changes in one place
- **DRY Principle**: Don't repeat yourself

## Risk Mitigation
- Ensure test rate limiter configuration works for all use cases
- Test that all existing tests continue to work
- Verify rate limiting behavior is preserved
- Keep function signature identical to avoid breaking changes

## Notes
Test utilities like `create_test_rate_limiter()` are cross-cutting infrastructure that should be centralized in the common crate. Having multiple duplicate implementations violates DRY principles and creates maintenance overhead.

This follows the same principle as moving other test utilities to swissarmyhammer-common - shared test infrastructure belongs in the common crate.