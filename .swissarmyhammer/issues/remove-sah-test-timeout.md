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