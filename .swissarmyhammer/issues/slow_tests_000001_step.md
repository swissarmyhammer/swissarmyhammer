# Step 1: Identify and Catalog Slow Tests

Refer to /Users/wballard/sah-slow_tests/ideas/slow_tests.md

## Objective
Run all tests in the SwissArmyHammer codebase to identify slow tests (>10 seconds per coding standards) and categorize them by root cause.

## Background
The SwissArmyHammer test suite has 1,739 tests across three cargo workspaces. Initial test run showed approximately 2 minutes total execution time, which suggests there are performance bottlenecks that need identification.

## Tasks

### 1. Run Comprehensive Test Analysis
- Use `cargo nextest run --fail-fast` to get detailed test timing data
- Generate a report of all tests with their execution times
- Identify tests taking >10 seconds (per CODING_STANDARDS.md)
- Create a temporary markdown file documenting slow tests

### 2. Categorize Slow Tests by Type
Create categories based on common patterns:
- **MCP Integration Tests**: Tests involving MCP server startup/communication
- **File System Heavy Tests**: Tests with extensive file I/O operations  
- **Serial Tests**: Tests marked with `#[serial]` preventing parallelization
- **Database Tests**: Tests using DuckDB for semantic search operations
- **Git Integration Tests**: Tests creating and manipulating repositories
- **E2E Workflow Tests**: Complete workflow execution tests
- **Performance Tests**: Benchmark and performance regression tests

### 3. Root Cause Analysis
For each slow test category, identify:
- Primary performance bottleneck (I/O, computation, network, etc.)
- Dependencies that prevent parallelization
- Opportunities for test splitting or optimization
- Mock opportunities to replace heavy operations

### 4. Document Findings
Create `/tmp/slow_test_analysis.md` containing:
- List of all slow tests with execution times
- Categorization by root cause
- Initial optimization recommendations
- Priority ordering for subsequent fixes

## Acceptance Criteria
- [ ] Complete test suite run with timing data captured
- [ ] All tests >10s identified and documented
- [ ] Tests categorized by performance bottleneck type
- [ ] Analysis document created with optimization recommendations
- [ ] No existing functionality broken during analysis

## Implementation Notes
- Use the existing `fast-test` cargo profile for compilation speed
- Focus on measurement and documentation, not fixes yet
- Maintain current test behavior and coverage
- Consider using `cargo nextest` for better timing output

## Estimated Effort
Small (1-2 focused work sessions)

## Dependencies
- None (initial analysis step)

## Follow-up Steps
This analysis will drive the subsequent optimization steps for each category of slow tests.