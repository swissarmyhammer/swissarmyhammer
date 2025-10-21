# Add max_errors Parameter to Rule Check Tool

## Summary
Add a new optional `max_errors` parameter to the rule check tool and CLI to enable early termination and chunked error processing.

## Requirements

### Parameter Specification
- **Name**: `max_errors`
- **Type**: Optional integer
- **Default**: None (check all violations)
- **Purpose**: Limit the number of errors returned by the rule checker

### Implementation Details

1. **CLI Switch**: Add `--max-errors` flag to the rules check CLI command
   - Should accept an integer value
   - Default to None if not specified
   - Example: `sah rules check --max-errors 5 src/**/*.rs`

2. **Tool Interface**: Add `max_errors` as an optional parameter to the `rules_check` tool

3. **Rule Checker Integration**: Pass `max_errors` through to the underlying rule checker options

4. **Early Termination**: The checker should abort once it reaches the `max_errors` threshold, avoiding unnecessary processing of remaining files

### Benefits
- Faster feedback when checking large codebases
- Ability to work on errors incrementally in chunks
- Reduced processing time when only a sample of errors is needed
- Better user experience for iterative fixing workflows

### Example Usage

**CLI:**
```bash
sah rules check --max-errors 5 --rule no-unwrap src/**/*.rs
```

**MCP Tool:**
```json
{
  "rule_names": ["no-unwrap"],
  "file_paths": ["src/**/*.rs"],
  "max_errors": 5
}
```

This would return up to 5 errors and then abort the check.

## Proposed Solution

After analyzing the codebase, I need to make changes in the following locations:

### 1. Core Domain Layer (swissarmyhammer-rules)
**File**: `swissarmyhammer-rules/src/checker.rs`
- Add `max_errors: Option<usize>` field to `RuleCheckRequest` struct (line 54)
- Modify the streaming `check()` method (line 535) to limit violations using `.take(max_errors)` when `max_errors` is Some

### 2. CLI Layer (swissarmyhammer-cli)
**File**: `swissarmyhammer-cli/src/commands/rule/cli.rs`
- Add `max_errors: Option<usize>` field to `CheckCommand` struct (line 33)
- Parse the `--max-errors` argument from clap matches (line 91)

**File**: `swissarmyhammer-cli/src/commands/rule/check.rs`
- Pass `max_errors` from CLI command to `RuleCheckRequest` (line 217)

### 3. MCP Tool Layer (swissarmyhammer-tools)
**File**: `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs`
- Add `max_errors: Option<usize>` field to MCP `RuleCheckRequest` struct (line 62)
- Update JSON schema to include `max_errors` parameter (line 168)
- Pass `max_errors` from MCP request to domain `RuleCheckRequest` (line 246)

### 4. Clap CLI Definition
Need to find and update the clap command builder to add the `--max-errors` argument

### Implementation Strategy

1. **Start with domain layer test**: Write failing test in `checker.rs` that verifies max_errors limits violations
2. **Implement domain layer**: Add field to `RuleCheckRequest` and modify stream to limit violations
3. **Update CLI layer**: Add field to `CheckCommand` and pass through to domain
4. **Update MCP layer**: Add field to MCP request and pass through to domain
5. **Update clap definition**: Add --max-errors argument to CLI parser
6. **Run integration tests**: Verify end-to-end functionality


## Implementation Notes

### Verification Date: 2025-10-21

The `max_errors` parameter has been **fully implemented** across all layers of the system. Here's what was found and verified:

#### 1. Domain Layer (swissarmyhammer-rules/src/checker.rs)
- **Line 70**: `max_errors: Option<usize>` field added to `RuleCheckRequest` struct
- **Lines 638-648**: Implementation logic using stream `.take(limit)` to limit violations
  - When `max_errors` is Some(n), the stream is limited to n violations
  - When `max_errors` is None and CheckMode is FailFast, stream is limited to 1
  - Otherwise, unlimited violations are returned
- **Lines 996-1049**: Comprehensive unit tests added:
  - `test_check_with_max_errors_limits_violations`: Verifies max_errors=2 returns at most 2 violations
  - `test_check_without_max_errors_unlimited`: Verifies max_errors=None returns all violations

**Test Result**: ✅ All 198 tests in swissarmyhammer-rules pass

#### 2. CLI Layer (swissarmyhammer-cli)

**cli.rs:**
- **Line 41**: `max_errors: Option<usize>` field added to `CheckCommand` struct
- **Line 116**: Parsing from clap matches using `get_one::<usize>("max-errors").copied()`

**check.rs:**
- **Line 224**: Passing `max_errors` from CLI command to domain `RuleCheckRequest`

**dynamic_cli.rs:**
- **Lines 1187-1192**: Clap argument definition for `--max-errors`
  - Long form: `--max-errors`
  - Help text: "Maximum number of ERROR violations to return (default: unlimited)"
  - Value parser: `clap::value_parser!(usize)`

**Test Fixes Applied:**
- Added `max-errors` argument definition to all 5 check command tests in cli.rs:
  - test_parse_check_command
  - test_parse_check_command_with_filters
  - test_parse_check_command_with_create_issues
  - test_parse_check_command_with_no_fail_fast
  - test_parse_check_command_with_both_flags

**Test Result**: ✅ All 147 CLI rule tests pass

#### 3. MCP Tool Layer (swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs)

- **Line 82**: `max_errors: Option<usize>` field added to MCP `RuleCheckRequest` struct
- **Lines 198-202**: JSON schema definition includes max_errors parameter:
  ```json
  "max_errors": {
    "type": "integer",
    "minimum": 1,
    "description": "Optional maximum number of ERROR violations to return (default: unlimited)"
  }
  ```
- **Line 265**: Passing `max_errors` from MCP request to domain `RuleCheckRequest`

### How It Works

The implementation uses Rust's `Stream::take()` method to limit the number of violations returned:

1. **Domain Layer**: The `RuleChecker::check()` method returns a stream of `RuleViolation` results
2. **Limiting Logic** (checker.rs:638-648):
   - If `max_errors` is specified, limit stream to that many violations
   - Otherwise, if `CheckMode::FailFast`, limit to 1 violation
   - Otherwise, return unlimited violations
3. **Early Termination**: The stream stops processing after reaching the limit, avoiding unnecessary file checks

### Usage Examples

**CLI:**
```bash
# Check and return up to 5 errors
sah rules check --max-errors 5 src/**/*.rs

# Check specific rule with error limit
sah rules check --rule no-unwrap --max-errors 10 src/**/*.rs

# Combine with other filters
sah rules check --severity error --max-errors 3 src/**/*.rs
```

**MCP Tool:**
```json
{
  "rule_names": ["no-unwrap"],
  "file_paths": ["src/**/*.rs"],
  "max_errors": 5
}
```

### Test Results Summary

- ✅ Domain layer tests: 198/198 passed (including 2 max_errors-specific tests)
- ✅ CLI layer tests: 147/147 passed (including 5 check command tests with max-errors)
- ✅ MCP schema: Properly defined with type validation

### Conclusion

The `max_errors` feature is **complete and working** as specified. All tests pass, and the parameter is correctly threaded through all three layers:
1. CLI argument parsing
2. MCP tool interface
3. Domain rule checking logic

The implementation enables faster feedback when checking large codebases and supports iterative fixing workflows.
