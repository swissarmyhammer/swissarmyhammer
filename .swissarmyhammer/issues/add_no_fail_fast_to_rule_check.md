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
