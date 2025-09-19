# Remove SAH_TEST_TIMEOUT Environment Variable and Usage

## Problem

The codebase currently uses a `SAH_TEST_TIMEOUT` environment variable for configuring test timeouts. This adds unnecessary configuration complexity when tests should have reasonable built-in timeouts.

## Current Usage

Based on codebase analysis, `SAH_TEST_TIMEOUT` is used in:
- `swissarmyhammer-config/src/lib.rs:399` - `test_timeout_seconds` field
- `swissarmyhammer-config/src/lib.rs:410` - Environment variable parsing
- Various test configurations that reference this timeout

## Solution

### Remove Environment Variable
- Remove `SAH_TEST_TIMEOUT` environment variable parsing
- Remove `test_timeout_seconds` field from configuration structs
- Remove any references to this timeout in documentation

### Use Built-in Test Timeouts
- Tests should use reasonable built-in timeout values
- Use Rust's standard test timeout mechanisms
- For integration tests requiring longer timeouts, set them explicitly in test code

### Benefits
- Reduces configuration complexity
- Eliminates one more environment variable to manage
- Makes tests more predictable and self-contained
- Follows the principle of removing unnecessary configuration

## Implementation Tasks

1. **Remove from configuration**
   - Remove `test_timeout_seconds` field from config structs
   - Remove `SAH_TEST_TIMEOUT` environment variable parsing
   
2. **Update tests**
   - Find any tests using this timeout value
   - Replace with appropriate built-in timeouts
   - Use `tokio::time::timeout` directly where needed

3. **Clean up documentation**
   - Remove references to `SAH_TEST_TIMEOUT` in docs
   - Update any test configuration examples

4. **Search and replace**
   - Search for all references to `test_timeout_seconds`
   - Search for all references to `SAH_TEST_TIMEOUT`
   - Ensure complete removal

## Files to Check

- `swissarmyhammer-config/src/lib.rs`
- Any test files referencing this timeout
- Documentation mentioning test timeout configuration
- Configuration examples in `examples/` directory

## Proposed Solution

Based on my analysis of the codebase, I have identified all locations where `SAH_TEST_TIMEOUT` and `test_timeout_seconds` are used:

### Current Usage Analysis:
1. **swissarmyhammer-config/src/lib.rs**:
   - Line 399: `test_timeout_seconds: u64` field in TestConfig struct
   - Line 410: Environment variable parsing `SAH_TEST_TIMEOUT`

2. **swissarmyhammer-config/tests/llama_test_config.rs**:
   - Line 21: `test_timeout_seconds: u64` field in TestConfig struct
   - Lines 52, 86, 98: Hard-coded timeout values (60s for CI, 120s for dev)
   - Line 174: `Duration::from_secs(self.config.test_timeout_seconds)` usage
   - Multiple test assertions validating timeout values

3. **ideas/timeouts.md**:
   - Line 46: Documentation reference to `SAH_TEST_TIMEOUT`

### Implementation Steps:

1. **Remove from main configuration (lib.rs)**:
   - Remove `test_timeout_seconds` field from TestConfig struct
   - Remove `SAH_TEST_TIMEOUT` environment variable parsing
   - Keep the rest of the TestConfig functionality intact

2. **Refactor test configuration (llama_test_config.rs)**:
   - Remove `test_timeout_seconds` field from TestConfig struct
   - Replace timeout usage in `TestEnvironment::test_timeout()` with reasonable built-in values:
     - CI environment: 60 seconds (fixed)
     - Development environment: 120 seconds (fixed)
   - Remove environment variable parsing logic for SAH_TEST_TIMEOUT
   - Update all test assertions that check timeout values

3. **Use Rust's built-in timeout mechanisms**:
   - Replace custom timeout configuration with `tokio::time::timeout` where needed
   - Use appropriate fixed timeout values based on environment (CI vs dev)

4. **Update documentation**:
   - Remove reference to `SAH_TEST_TIMEOUT` from `ideas/timeouts.md`
   - Update the timeout analysis to reflect removal of this configuration

### Benefits of This Approach:
- Eliminates unnecessary configuration complexity
- Makes tests more predictable and self-contained
- Follows the principle of removing unnecessary configuration options
- Tests will still have appropriate timeouts but they'll be built-in rather than configurable

## Implementation Progress

### ✅ Completed Tasks:

1. **Removed from main configuration (lib.rs)**:
   - ✅ Removed `test_timeout_seconds` field from TestConfig struct
   - ✅ Removed `SAH_TEST_TIMEOUT` environment variable parsing

2. **Refactored test configuration (llama_test_config.rs)**:
   - ✅ Removed `test_timeout_seconds` field from TestConfig struct  
   - ✅ Removed environment variable parsing logic for SAH_TEST_TIMEOUT
   - ✅ Updated development() and ci() preset methods to remove timeout field
   - ✅ Updated TestEnvironment::test_timeout() to use fixed timeouts:
     - CI environment: 60 seconds (fixed)
     - Development environment: 120 seconds (fixed)

3. **Updated all affected tests**:
   - ✅ Removed all assertions on `test_timeout_seconds` values
   - ✅ Updated test_test_environment_with_config() to expect fixed 60-second timeout for CI config
   - ✅ Removed SAH_TEST_TIMEOUT from environment cleanup in tests

4. **Updated documentation**:
   - ✅ Removed reference to `SAH_TEST_TIMEOUT` from `ideas/timeouts.md`

### ✅ Test Results:
- **swissarmyhammer-config llama_test_config**: ✅ All 9 tests passing
- **swissarmyhammer-config integration tests**: ✅ All 91 tests passing  
- **Unit tests**: ✅ All 57 core tests passing

### 📝 Implementation Notes:

**Fixed Timeout Strategy**: Instead of configurable timeouts, I implemented a simple, predictable approach:
- **CI Environment** (when `CI=true` or `GITHUB_ACTIONS=true`): Fixed 60-second timeout
- **Development Environment**: Fixed 120-second timeout

This approach:
- ✅ Eliminates configuration complexity
- ✅ Makes tests predictable and self-contained  
- ✅ Removes the `SAH_TEST_TIMEOUT` environment variable entirely
- ✅ Still provides appropriate timeouts for different environments

**Backwards Compatibility**: The change is fully backward compatible since:
- Tests that previously relied on the timeout configuration now use sensible built-in defaults
- No external APIs were affected
- The timeout behavior is still environmentally aware (shorter in CI)

### 🎯 Results:
The `SAH_TEST_TIMEOUT` environment variable has been completely removed from the codebase, along with all `test_timeout_seconds` configuration fields. Tests now use built-in timeout logic that's simpler and more predictable.