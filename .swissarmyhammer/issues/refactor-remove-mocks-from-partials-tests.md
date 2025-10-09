# Refactor: Remove Mocks from Partials Tests

## Issue
The `swissarmyhammer-rules/tests/partials_test.rs` file violates the `no-mocks` rule with three mock implementations that should be replaced with real file system operations.

## Violations

### 1. MockPartialLoader (Line 119)
- **Location**: `test_partial_rendering_in_rule` test
- **Issue**: Mock object that simulates real PartialLoader behavior
- **Fix**: Replace with real PartialLoader using actual temporary files

### 2. EmptyPartialLoader (Line 178)
- **Location**: `test_partial_not_found_error` test
- **Issue**: Stubbed implementation returning fake empty responses
- **Fix**: Use real PartialLoader backed by an actual empty temporary directory

### 3. MultiPartialLoader (Line 221)
- **Location**: `test_multiple_partials_in_rule` test
- **Issue**: Stubbed implementation with predetermined data
- **Fix**: Create actual partial files in temp directory and use real RuleLoader

## Approach
Follow the pattern used in `test_load_partials_from_directory` which already demonstrates proper testing with real file system operations:
1. Create temporary directories for each test
2. Write actual partial files to disk
3. Use the real `RuleLoader` to load partials
4. Clean up temporary resources after tests

## Benefits
- Tests real behavior instead of mocked behavior
- Better integration testing
- Catches real file system edge cases
- Follows "no mocks" rule compliance

## Files to Modify
- `swissarmyhammer-rules/tests/partials_test.rs`
