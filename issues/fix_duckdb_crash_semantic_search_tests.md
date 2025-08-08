# Fix DuckDB Crash in Semantic Search Tests

In making search tests pass, index no more than 6 files to avoid timeouts.


## Location
Multiple locations in `swissarmyhammer-cli/src/search.rs`:
- Line 409: `test_run_semantic_index_empty_patterns`
- Line 421: `test_run_semantic_index_single_pattern`
- Line 449: `test_run_semantic_index_multiple_patterns`

Also in `swissarmyhammer-cli/tests/search_cli_test.rs`:
- Line 86: `test_search_query`

## Current State
Multiple semantic search tests are marked with `#[ignore]` due to "DuckDB crash during cleanup". This prevents testing of critical search functionality.

## Root Cause Analysis Needed
- Investigate the DuckDB crash during test cleanup
- Could be related to:
  - Database connection not being properly closed
  - Resource cleanup order issues
  - Concurrent access during teardown
  - Memory management issues with DuckDB
  - File locking on the database file

## Requirements
1. Diagnose the root cause of the DuckDB crash
2. Fix the underlying issue causing the crash
3. Ensure proper cleanup of DuckDB resources
4. Re-enable all affected tests
5. Ensure tests run reliably without crashes
6. Tests should complete in under 10 seconds per coding standards

## Investigation Steps
1. Run tests individually to isolate crash conditions
2. Check DuckDB version compatibility
3. Review database connection lifecycle management
4. Verify proper async/await handling with DuckDB
5. Check for file system cleanup issues

## Acceptance Criteria
- [ ] Root cause of DuckDB crash identified
- [ ] Fix implemented for proper DuckDB cleanup
- [ ] All `#[ignore]` attributes removed from affected tests
- [ ] Tests pass consistently without crashes
- [ ] Tests complete within performance requirements
- [ ] No resource leaks or hanging processes
- [ ] CI/CD pipeline runs these tests successfully