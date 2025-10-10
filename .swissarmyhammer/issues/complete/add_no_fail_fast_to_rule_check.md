# Add --no-fail-fast Flag to sah rule check Command

## Objective

Add a `--no-fail-fast` flag to the `sah rule check` command to continue checking all rules and files even when violations are found, instead of exiting early on first error.

## Context

When using `--create-issues`, we need to check all files and rules to create issues for all ERROR violations, not just the first one encountered. This requires running the complete check without early exit.

## Requirements

### CLI Flag

- Add `--no-fail-fast` flag to `sah rule check` command
- Inspired by cargo's `cargo test --no-fail-fast` behavior
- When enabled, continue checking all rules against all files even when violations are found
- When disabled (default), maintain current behavior of potentially exiting early

### Implementation

1. **Add CLI Argument**
   - Add `--no-fail-fast` boolean flag to `RuleCheckArgs` in `swissarmyhammer-cli/src/commands/rule/cli.rs`
   - Default value: `false` (maintain current behavior)

2. **Update RuleCheckRequest**
   - Add `no_fail_fast: bool` field to `RuleCheckRequest` in `swissarmyhammer-rules/src/checker.rs`
   - Pass this flag through from CLI to the rule checking logic

3. **Modify Check Logic**
   - Update rule checking loop to respect `no_fail_fast` flag
   - When `no_fail_fast` is true, collect all violations and continue checking
   - When `no_fail_fast` is false, maintain current early-exit behavior
   - Ensure all violations are collected before reporting

4. **Integration with --create-issues**
   - When `--create-issues` is specified, automatically enable `no_fail_fast` behavior
   - This ensures all violations are found and issues are created for all of them
   - Can be explicit or implicit: `--create-issues` implies `--no-fail-fast`

### Behavior

```bash
# Default behavior - may exit early on errors
sah rule check

# Continue checking all files even with errors
sah rule check --no-fail-fast

# Automatically runs with no-fail-fast when creating issues
sah rule check --create-issues

# Explicit combination (redundant but allowed)
sah rule check --create-issues --no-fail-fast
```

## Implementation Details

### Current Early Exit Points

Identify and modify places in the code where checking stops early:
- File-level error handling
- Rule-level error handling
- Violation accumulation logic

### Error Collection

- Continue collecting violations even when errors occur
- Store all violations in a collection
- Report all violations at the end instead of as they're found
- Maintain proper error counting and severity tracking

### Exit Code

- Exit code should reflect the highest severity found
- ERROR violations → non-zero exit code
- WARNING/INFO/HINT violations → zero exit code (or configurable)
- Respect `no_fail_fast` flag but still return appropriate exit code

## Testing Requirements

- Test that `--no-fail-fast` checks all files
- Test that default behavior is maintained without flag
- Test that `--create-issues` automatically enables no-fail-fast
- Test that all violations are reported with no-fail-fast
- Test correct exit codes with multiple violations

## Files to Modify

- `swissarmyhammer-cli/src/commands/rule/cli.rs` - Add CLI argument
- `swissarmyhammer-cli/src/commands/rule/check.rs` - Pass flag to checker, integrate with --create-issues
- `swissarmyhammer-rules/src/checker.rs` - Add field to RuleCheckRequest, modify check logic
- Tests for both CLI and core checking logic

## Acceptance Criteria

- [ ] `--no-fail-fast` flag added to CLI
- [ ] `no_fail_fast` field added to `RuleCheckRequest`
- [ ] Rule checking respects the flag and continues on errors when enabled
- [ ] Default behavior (fail-fast) is maintained
- [ ] `--create-issues` automatically enables no-fail-fast behavior
- [ ] All violations are collected and reported correctly
- [ ] Correct exit codes returned
- [ ] Tests cover all scenarios
- [ ] Documentation updated

## Related Issues

- Depends on or works with: `add_create_issues_flag_to_rule_check`



## Proposed Solution

After analyzing the codebase, I've identified the implementation approach:

### Current Architecture

The rule checking system has two modes already implemented:
1. **Fail-fast mode** (`check_with_filters`) - exits on first ERROR violation
2. **Collection mode** (`check_with_filters_collect`) - collects all ERROR violations

The `--create-issues` flag already uses collection mode internally. We need to expose this behavior through a new `--no-fail-fast` flag.

### Implementation Steps

1. **CLI Layer** (`swissarmyhammer-cli/src/commands/rule/cli.rs`)
   - Add `no_fail_fast: bool` field to `CheckCommand` struct
   - Parse the flag in `parse_rule_command()` using `get_flag("no-fail-fast")`

2. **Command Builder** (`swissarmyhammer-cli/src/dynamic_cli.rs`)
   - Add the `--no-fail-fast` argument definition to the check subcommand around line 1310
   - Use `ArgAction::SetTrue` pattern like `--create-issues`

3. **Request Layer** (`swissarmyhammer-rules/src/checker.rs`)
   - Add `no_fail_fast: bool` field to `RuleCheckRequest` struct
   - This will be passed from CLI to the checker

4. **Execution Layer** (`swissarmyhammer-cli/src/commands/rule/check.rs`)
   - In `execute_check_command_impl`, update the `RuleCheckRequest` construction to include `no_fail_fast`
   - Modify the logic to choose between fail-fast and collection mode based on `no_fail_fast || create_issues`
   - When either flag is true, use `check_with_filters_collect()`, otherwise use `check_with_filters()`

5. **Integration with --create-issues**
   - The logic `no_fail_fast || create_issues` means:
     - `--create-issues` automatically enables no-fail-fast behavior
     - `--no-fail-fast` alone runs checks without creating issues
     - Both flags together is valid (redundant but allowed)

### Key Design Decisions

- **Reuse existing collection mode**: The `check_with_filters_collect()` method already implements the desired behavior
- **Logical OR for mode selection**: `no_fail_fast || create_issues` ensures both flags enable collection mode
- **No changes to checker logic**: All the plumbing already exists, we just expose it through CLI
- **Default behavior preserved**: Without flags, continues to fail-fast on ERROR violations

### Testing Strategy

1. Test CLI parsing of `--no-fail-fast` flag
2. Test that `no_fail_fast` field is correctly passed through to `RuleCheckRequest`
3. Test that collection mode is used when flag is set
4. Test that `--create-issues` still works (implies no-fail-fast)
5. Test exit codes are correct with multiple violations



## Implementation Notes

### Changes Made

1. **CLI Layer** (`swissarmyhammer-cli/src/commands/rule/cli.rs`)
   - Added `no_fail_fast: bool` field to `CheckCommand` struct
   - Updated `parse_rule_command()` to parse the flag using `get_flag("no-fail-fast")`
   - Updated all existing tests to include the new field
   - Added three new tests:
     - `test_parse_check_command_with_no_fail_fast` - tests flag alone
     - `test_parse_check_command_with_both_flags` - tests both flags together

2. **Command Builder** (`swissarmyhammer-cli/src/dynamic_cli.rs`)
   - Added `--no-fail-fast` argument definition after `--create-issues`
   - Uses `ArgAction::SetTrue` pattern for boolean flag
   - Added help text: "Continue checking all rules and files even when violations are found"

3. **Request Layer** (`swissarmyhammer-rules/src/checker.rs`)
   - Added `no_fail_fast: bool` field to `RuleCheckRequest` struct
   - Updated documentation example to include the new field
   - Updated all tests to include `no_fail_fast: false` in request initialization

4. **Execution Layer** (`swissarmyhammer-cli/src/commands/rule/check.rs`)
   - Updated `RuleCheckRequest` construction to include `no_fail_fast` from CLI
   - Modified condition to use `no_fail_fast || create_issues` for collection mode
   - Updated output messages to distinguish between:
     - `--create-issues` mode: "Found X ERROR violation(s), creating issues..."
     - `--no-fail-fast` mode: "Found X ERROR violation(s)"
   - Added logic to return error with non-zero exit code when violations are found
   - Only create issues when `--create-issues` flag is explicitly set
   - Updated all 7 existing tests to include the new field
   - Added 2 new tests:
     - `test_execute_check_command_with_no_fail_fast_flag`
     - `test_execute_check_command_with_both_flags`

5. **MCP Tools Integration** (`swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs`)
   - Added `no_fail_fast: false` to `DomainRuleCheckRequest` construction
   - MCP tool uses default fail-fast behavior (no flag exposed in MCP interface)

### Behavior Summary

- **Default** (`sah rule check`): Fail-fast mode, exits on first ERROR violation
- **With `--no-fail-fast`**: Collection mode, checks all files, reports all ERROR violations, exits with error if any found
- **With `--create-issues`**: Collection mode (implicit no-fail-fast), creates issues for all ERROR violations
- **With both flags**: Collection mode, creates issues for all ERROR violations (redundant but allowed)

### Test Results

All 3,336 tests passed successfully, including:
- 5 slow tests (> 5s) in LLM integration areas
- All new CLI parsing tests
- All updated execution tests
- All checker library tests

### Exit Code Behavior

- Exit code 0: All checks passed, no ERROR violations
- Exit code 1: ERROR violations found (with or without --no-fail-fast)
- The --no-fail-fast flag changes collection behavior but maintains correct exit codes
