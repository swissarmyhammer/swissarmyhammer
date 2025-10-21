# Fix --max-errors Default Value

## Problem
The `--max-errors` CLI parameter currently shows "default: unlimited" in the help text, but it should default to 1 instead.

## Current Behavior
```
--max-errors <N>
    Maximum number of ERROR violations to return (default: unlimited)
```

## Expected Behavior
```
--max-errors <N>
    Maximum number of ERROR violations to return (default: 1)
```

## Requirements

1. **Change Default Value**: Update the default value from unlimited to 1
2. **Update Help Text**: Ensure the help text accurately reflects the new default
3. **Update Implementation**: Modify the CLI argument parser to use 1 as the default when `--max-errors` is not specified
4. **Consistency**: Ensure this matches the behavior specified in the related `max-errors-rule-check` issue

## Rationale
A default of 1 enables:
- Faster feedback by returning quickly with the first error
- Incremental/chunked error fixing workflow
- Reduced processing time for large codebases
- Users can still opt-in to see all errors by explicitly setting a higher value or removing the limit

## Related Issues
- `max-errors-rule-check` - Initial issue for adding the max_errors parameter
## Proposed Solution

After analyzing the codebase, I've identified the locations that need to be changed:

### Files to Modify

1. **swissarmyhammer-cli/src/dynamic_cli.rs** (line ~1190)
   - Update the help text from "default: unlimited" to "default: 1"
   - Add `.default_value("1")` to the Arg builder

2. **swissarmyhammer-cli/src/commands/rule/cli.rs** (line 41)
   - The `CheckCommand.max_errors` field is already `Option<usize>`, which is correct
   - The parsing logic at line 116 uses `.copied()` which will use None if not provided
   - Need to handle the default value of 1 when None is parsed

3. **swissarmyhammer-cli/src/commands/rule/check.rs** (line 224)
   - This passes `max_errors` to the rules checker
   - Need to ensure we use `Some(1)` when the CLI provides None

### Implementation Steps

1. **Write a failing test** that verifies:
   - When --max-errors is NOT specified, the default value is 1
   - When --max-errors IS specified, that value is used
   - The help text shows "default: 1"

2. **Update dynamic_cli.rs**:
   - Change help text to show "default: 1"
   - Add `.default_value("1")` to ensure clap provides the default

3. **Verify the parsing logic**:
   - Check that parse_rule_command correctly handles the default value
   - The current implementation uses `.copied()` which should work with default_value

4. **Run all tests** to ensure nothing breaks

### Test Strategy

Following TDD:
1. Create a test that checks the default value is 1 when --max-errors is not specified
2. Run test to see it fail
3. Implement the fix
4. Run test to see it pass
5. Run all tests to ensure no regressions

## Implementation Notes

### Changes Made

1. **Added tests in swissarmyhammer-cli/src/commands/rule/cli.rs**:
   - `test_parse_check_command_max_errors_defaults_to_one()`: Verifies default value is Some(1)
   - `test_parse_check_command_max_errors_explicit_value()`: Verifies explicit value overrides default

2. **Updated swissarmyhammer-cli/src/dynamic_cli.rs** (line ~1189):
   - Changed help text from "default: unlimited" to "default: 1"
   - Added `.default_value("1")` to the Arg builder
   - This ensures clap provides the default value when --max-errors is not specified

### How It Works

When the user does NOT specify --max-errors:
- Clap provides the default value "1" to the parser
- The parser converts it to Some(1) via `.copied()`
- The rule checker receives Some(1) and limits ERROR violations to 1

When the user DOES specify --max-errors N:
- Clap uses the user-provided value
- The parser converts it to Some(N)
- The rule checker receives Some(N) and limits ERROR violations to N

### Files Modified

- `swissarmyhammer-cli/src/commands/rule/cli.rs`: Added 2 tests
- `swissarmyhammer-cli/src/dynamic_cli.rs`: Updated help text and added default_value
