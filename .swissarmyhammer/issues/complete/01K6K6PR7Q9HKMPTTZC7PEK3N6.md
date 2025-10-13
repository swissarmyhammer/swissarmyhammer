i do not want a --code flag on rule check, we will only check files


## Proposed Solution

The --code flag needs to be removed from the `rule check` command. This involves:

1. **CheckCommand struct** (cli.rs): Remove the `code` field
2. **CLI argument parser** (cli.rs): Remove code argument parsing in parse_rule_command
3. **Dynamic CLI builder** (dynamic_cli.rs): Remove .arg for code flag
4. **Execute function** (check.rs): 
   - Remove validation that checks for --code conflicts
   - Remove temp file creation logic for inline code
   - Simplify to only handle file patterns
5. **Tests** (check.rs): Remove tests for --code functionality
6. **Documentation** (description.md): Remove --code examples and usage

The implementation is straightforward - we're removing functionality rather than adding it, so we just need to delete the relevant code paths and ensure the remaining file-based checking still works correctly.


## Implementation Complete

Successfully removed the `--code` flag from the `rule check` command. All changes compiled successfully and all 3223 tests passed.

### Changes Made

1. **CheckCommand struct** (cli.rs:27): Removed `code: Option<String>` field
2. **parse_rule_command** (cli.rs:62): Removed code field from CheckCommand initialization
3. **Dynamic CLI builder** (dynamic_cli.rs:1248): 
   - Removed `--code` argument definition
   - Updated long_about help text to remove `--code` usage examples
   - Removed the second usage line mentioning `--code`
4. **execute_check_command** (check.rs:175):
   - Removed validation checks for `--code` conflicts
   - Removed temp file creation logic for inline code
   - Simplified to only handle file patterns via `expand_glob_patterns`
   - Updated function documentation to remove `--code` example
5. **Tests** (check.rs:680):
   - Removed `test_execute_check_command_with_inline_code`
   - Removed `test_execute_check_command_inline_code_requires_rule`
   - Removed `test_execute_check_command_code_and_patterns_mutually_exclusive`
   - Updated all remaining tests to remove `code: None` field references
6. **Tests** (cli.rs:180): Updated test command builders to remove `--code` arg
7. **Tests** (mod.rs:120): Updated CheckCommand instantiation to remove `code: None`
8. **Documentation** (description.md:79):
   - Removed `--code CODE` from options list
   - Removed inline code checking example
   - Removed workflow example for testing rules against inline code

### Test Results

All 3223 tests passed successfully, confirming the implementation is correct and no functionality was broken.


## Code Review Completed

Ran comprehensive code review on all changed files. Results:

- ✅ All 3223 tests passing
- ✅ No clippy warnings or errors  
- ✅ No issues found in code review
- ✅ All documentation updated correctly
- ✅ No placeholders, TODOs, or commented code
- ✅ Proper error handling throughout
- ✅ 30+ tests covering changed functionality

The implementation successfully removes the `--code` flag from `rule check` command with no issues identified.

Files reviewed:
1. swissarmyhammer-cli/src/commands/rule/check.rs
2. swissarmyhammer-cli/src/commands/rule/cli.rs
3. swissarmyhammer-cli/src/commands/rule/description.md
4. swissarmyhammer-cli/src/commands/rule/mod.rs
5. swissarmyhammer-cli/src/dynamic_cli.rs

CODE_REVIEW.md has been removed.