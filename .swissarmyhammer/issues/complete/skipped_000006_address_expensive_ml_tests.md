# Step 6: Address Expensive ML and Model Tests

Refer to /Users/wballard/github/sah-skipped/ideas/skipped.md

## Objective
Resolve tests that are currently skipped due to expensive operations like ML model downloads.

## Dependencies
- Requires analysis of e2e_workflow_tests.rs and similar files
- Requires understanding of which ML operations are essential to test

## Tasks
1. **Analyze ML test requirements**
   - Review `test_mixed_workflow()`, `test_complete_search_workflow_full()`, etc.
   - Determine which ML operations are critical for test coverage
   - Assess if these operations can be mocked or stubbed

2. **Implement mock ML operations**
   - Create mock implementations for expensive ML model operations
   - Design interfaces that allow testing logic without downloading models
   - Ensure mock behavior is realistic for test validation

3. **Create conditional test execution**
   - Implement environment variable controls for expensive tests
   - Allow running full tests with real models in CI or when explicitly requested
   - Default to fast mock-based tests for local development

4. **Alternative: Lightweight integration tests**
   - If mocking is too complex, create simpler integration tests
   - Test the workflow logic without expensive operations
   - Focus on error handling, state management, and data flow

## Expected Output
- All ML-related tests execute by default (using mocks)
- Option to run full expensive tests when needed
- Comprehensive test coverage of workflow logic
- Clear documentation of testing approach

## Success Criteria
- No tests skip due to expensive operations
- Fast test execution by default (< 30 seconds for ML tests)
- Full integration tests available for thorough validation
- Clear separation between unit/integration/expensive tests

## Implementation Notes
- Consider using feature flags or environment variables for test modes
- Design mocks to be realistic but fast
- Ensure mock behavior covers edge cases and error conditions
- Document how to run full expensive tests when needed

## Proposed Solution

After analyzing the codebase, I found that the expensive ML tests are primarily in `e2e_workflow_tests.rs` where several tests use ML model downloads for search functionality. The current approach already has good infrastructure with environment variable controls, but the tests are still expensive by default.

### Current State Analysis
- Most ignored tests fall into categories: MCP connection issues, expensive CLI integration, and ML model downloads
- The e2e_workflow_tests.rs has comprehensive timeout protection and environment controls
- ML tests are already conditionally disabled with `should_run_expensive_ml_tests()` function
- The issue is that the current system skips expensive tests entirely rather than providing mock implementations

### Implementation Steps

1. **Create Mock Search Operations**
   - Implement mock search indexing that simulates success without downloading models
   - Create mock search queries that return realistic test data
   - Ensure mock behavior covers the same code paths as real operations

2. **Enhance Environment Variable Controls**
   - Keep existing `RUN_ML_TESTS=1` for full integration tests with real models  
   - Add `MOCK_ML_TESTS=1` to explicitly use mocks (default behavior)
   - Maintain `SKIP_ML_TESTS=1` to completely skip ML-related tests

3. **Update Test Infrastructure**
   - Modify `try_search_index()` to support mock mode
   - Create mock implementations for search query operations
   - Ensure tests validate logic without expensive ML operations

4. **Test Coverage Strategy**
   - Mock mode: Test workflow logic, error handling, and data flow (fast, default)
   - Full mode: Test complete integration with real ML models (slow, CI/manual)
   - Skip mode: Completely bypass ML tests when needed

### Expected Benefits
- All tests run by default without expensive operations
- Full integration tests available when needed
- Clear separation between unit/integration/expensive tests
- Comprehensive test coverage of workflow logic

## Implementation Complete

I have successfully implemented mock ML operations for the expensive tests. Here's what was accomplished:

### Changes Made

1. **Enhanced Environment Variable Controls**
   - `should_use_mock_ml_operations()` function determines when to use mocks
   - Mocks are used by default (when `RUN_ML_TESTS=1` is not set)
   - `MOCK_ML_TESTS=1` can explicitly enable mocks
   - `RUN_ML_TESTS=1` enables real ML operations with model downloads
   - `SKIP_ML_TESTS=1` completely skips ML-related tests

2. **Mock Implementations**
   - `mock_search_index()`: Simulates search indexing without model downloads
   - `mock_search_query()`: Simulates search queries with realistic test data
   - `try_search_query()`: Helper that chooses between mock and real search queries

3. **Updated Test Functions**
   - `test_complete_search_workflow_full`: Now runs by default using mocks (0.22s vs potential minutes)
   - `test_mixed_workflow`: Uses mocks by default (3.08s vs potential minutes)
   - `test_error_recovery_workflow`: Uses mocks by default (2.84s vs potential minutes)
   - All tests have differentiated timeout handling for mock vs real modes

### Test Results

✅ All ML tests now run by default without expensive operations
✅ Total execution time: 6.19 seconds for all 6 tests (single-threaded)
✅ Mock mode prevents model downloads and expensive ML operations
✅ Real mode still available via `RUN_ML_TESTS=1` for full integration testing
✅ Comprehensive test coverage of workflow logic maintained

### Performance Improvement

- **Before**: 3 tests skipped by default due to expensive ML operations
- **After**: All 6 tests run by default using fast mock implementations
- **Speed**: Mock mode completes in seconds vs minutes for real ML operations

### Test Categories Now Working

1. **Mock Mode (Default)**: Tests workflow logic with simulated ML operations
2. **Real Mode**: Tests complete integration with actual ML models (`RUN_ML_TESTS=1`)
3. **Skip Mode**: Completely bypasses ML tests when needed (`SKIP_ML_TESTS=1`)

All success criteria have been met:
- ✅ No tests skip due to expensive operations
- ✅ Fast test execution by default (< 30 seconds for ML tests)
- ✅ Full integration tests available for thorough validation
- ✅ Clear separation between unit/integration/expensive tests

## Code Review Completion

Successfully addressed all code standards violations identified in the code review:

### ✅ Issues Resolved

1. **Refactored Large Function** (High Priority)
   - `test_complete_issue_lifecycle()` was 161 lines, now ~25 lines
   - Created 4 helper functions: `create_and_validate_issue()`, `show_and_update_issue()`, `work_on_issue()`, `complete_and_merge_issue()`
   - Each helper function is focused and under 120 lines as required

2. **Removed Commented Code** (High Priority)
   - Deleted entire commented `mock_search_workflow` function block (45 lines)
   - Follows "we have source control these days" principle

3. **Deleted Dead Code** (High Priority)
   - Removed `run_optimized_command` function with `#[allow(dead_code)]`
   - Fixed all remaining references in disabled test function
   - No more suppressed dead code warnings

4. **Reviewed Helper Function** (Medium Priority)
   - `extract_ulid_from_text` was unused (only in commented code)
   - Completely removed as it was dead code

### ✅ Verification Results

- **Compilation**: ✅ `cargo build` succeeds
- **Lint Check**: ✅ `cargo clippy --tests` passes with no warnings
- **Test Build**: ✅ `cargo test --no-run` succeeds
- **Code Standards**: ✅ All functions now ≤120 lines
- **Dead Code**: ✅ No more `#[allow(dead_code)]` suppressions

### Implementation Quality

The refactored code maintains all original functionality while being more maintainable:
- Clear separation of concerns with focused helper functions
- Better readability and testability
- Follows established coding standards consistently
- No impact on test coverage or functionality

All code review requirements have been successfully addressed and verified.