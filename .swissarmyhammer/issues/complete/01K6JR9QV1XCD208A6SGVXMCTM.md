# Eliminate `rule test` command - consolidate into `rule check`

## Problem
The `rule test` and `rule check` commands are nearly identical in functionality, creating unnecessary duplication and user confusion.

## Current State

### `rule check`
- Runs rules against code files
- Takes file patterns to check
- Can filter by rule, severity, category
- Reports violations

### `rule test`  
- Tests a specific rule with sample code
- Takes a rule name (required)
- Takes either `--file` or `--code` for input
- Reports violations

## The Duplication
Both commands:
- Load and execute rules
- Analyze code
- Report violations
- Have similar CLI interfaces
- Share most of the underlying implementation

## Proposed Solution

### Eliminate `rule test` command entirely

### Enhance `rule check` with optional `--rule` filter
```bash
# Current behavior - check all rules against files
sah rule check "src/**/*.rs"

# New behavior - check specific rule(s) against files  
sah rule check "src/**/*.rs" --rule no-hardcoded-secrets
sah rule check "src/**/*.rs" --rule no-hardcoded-secrets --rule sql-injection

# Check specific rule against inline code (replaces rule test)
sah rule check --code "fn main() { let api_key = \"sk-1234\"; }" --rule no-hardcoded-secrets

# Check specific rule against a file (replaces rule test)
sah rule check test.rs --rule no-hardcoded-secrets
```

## Benefits

1. **Simpler CLI** - One command instead of two
2. **Less confusion** - Clear that you're "checking" code with rules
3. **More flexible** - Can check multiple specific rules at once
4. **Less code** - Remove duplicate implementation
5. **Better UX** - Single mental model for running rules

## Implementation Changes

### 1. Update `rule check` command
- `--rule` already exists and supports multiple values
- Add `--code` option to accept inline code (from `rule test`)
- When `--code` is provided, create temporary file or pass to rules directly
- Keep existing file pattern behavior

### 2. Remove `rule test` command
- Delete `swissarmyhammer-cli/src/commands/rule/test.rs`
- Remove from CLI definition in `dynamic_cli.rs`
- Remove from command routing in `mod.rs`
- Update `RuleCommand` enum to remove `Test` variant

### 3. Update documentation
- Remove references to `rule test`
- Update help text for `rule check` to show `--code` option
- Update examples to show new usage patterns

### 4. Migration guide
Add to docs:
```markdown
## Migrating from `rule test`

Old:
sah rule test no-hardcoded-secrets --file test.rs
sah rule test no-hardcoded-secrets --code "fn main() {}"

New:  
sah rule check test.rs --rule no-hardcoded-secrets
sah rule check --code "fn main() {}" --rule no-hardcoded-secrets
```

## Files to Modify

- `swissarmyhammer-cli/src/commands/rule/test.rs` - DELETE
- `swissarmyhammer-cli/src/commands/rule/check.rs` - ADD `--code` option
- `swissarmyhammer-cli/src/commands/rule/cli.rs` - Remove `Test` variant, update `CheckCommand`
- `swissarmyhammer-cli/src/commands/rule/mod.rs` - Remove test module and routing
- `swissarmyhammer-cli/src/dynamic_cli.rs` - Remove `test` subcommand definition
- Tests - Migrate `rule test` tests to `rule check` tests

## Breaking Change Note
This is a breaking change that removes the `rule test` command. Users will need to migrate to `rule check` with the `--rule` flag.

## Validation
- [ ] All `rule test` functionality available via `rule check`
- [ ] `--code` option works for inline code checking
- [ ] `--rule` option filters to specific rules
- [ ] Multiple `--rule` flags work correctly
- [ ] Tests migrated and passing
- [ ] Documentation updated



## Detailed Implementation Analysis

### Current Architecture
I've reviewed the codebase and here's what currently exists:

1. **test.rs** (swissarmyhammer-cli/src/commands/rule/test.rs:183)
   - Provides `execute_test_command()` which handles rule testing with `--file` or `--code`
   - Performs full diagnostic output through multiple phases
   - Has complete test suite

2. **check.rs** (swissarmyhammer-cli/src/commands/rule/check.rs:293)
   - Provides `execute_check_command()` which runs rules against file patterns
   - Already has `--rule` filter that can take multiple rules
   - Uses glob pattern expansion with gitignore support
   - Missing: `--code` option for inline code

3. **cli.rs** (swissarmyhammer-cli/src/commands/rule/cli.rs:367)
   - Defines `TestCommand` struct with `rule_name`, `file`, `code`
   - Defines `CheckCommand` struct with `patterns`, `rule`, `severity`, `category`
   - Contains `RuleCommand` enum with both `Test` and `Check` variants

4. **mod.rs** (swissarmyhammer-cli/src/commands/rule/mod.rs:49)
   - Routes commands to their execution functions
   - Contains tests for all command routing

5. **dynamic_cli.rs** (swissarmyhammer-cli/src/dynamic_cli.rs:1138-1400)
   - Lines 1333-1399 define the `test` subcommand in `build_rule_command()`
   - This static definition needs to be removed

### Consolidation Strategy

The key insight is that `test` is essentially `check` with:
- A single specific rule (via `--rule`)
- Either a file pattern OR inline code (via `--code`)

So we need to:
1. Add `--code` option to `CheckCommand`
2. When `--code` is provided, treat it as a temporary file for checking
3. Remove `TestCommand` and the test subcommand entirely
4. Migrate all test functionality into check

### Implementation Plan

#### Phase 1: Enhance CheckCommand
1. Add `code: Option<String>` field to `CheckCommand` struct in cli.rs
2. Update `parse_rule_command()` in cli.rs to parse the new `--code` argument
3. Add `--code` argument to the check subcommand in dynamic_cli.rs

#### Phase 2: Enhance execute_check_command
1. Modify `execute_check_command()` in check.rs to handle `--code` option
2. When `--code` is provided:
   - Create a temporary PathBuf with appropriate extension
   - Skip glob expansion
   - Use the inline code directly as file content
3. Ensure error handling when both patterns and --code are provided

#### Phase 3: Remove Test Command
1. Delete test.rs entirely
2. Remove `Test(TestCommand)` variant from `RuleCommand` enum in cli.rs
3. Remove test module import and routing in mod.rs
4. Remove test subcommand from `build_rule_command()` in dynamic_cli.rs
5. Remove test parsing logic from `parse_rule_command()` in cli.rs

#### Phase 4: Migrate Tests
1. Convert test.rs unit tests to check.rs tests with --code option
2. Update mod.rs tests to remove test command routing tests
3. Ensure all test coverage is maintained

## Proposed Solution Details

### New CheckCommand Behavior

```bash
# Existing behavior - unchanged
sah rule check "src/**/*.rs"
sah rule check "src/**/*.rs" --rule no-hardcoded-secrets

# New behavior - inline code checking (replaces test command)
sah rule check --code "fn main() { let api_key = \"sk-1234\"; }" --rule no-hardcoded-secrets

# New behavior - single file with specific rule (replaces test command)  
sah rule check test.rs --rule no-hardcoded-secrets
```

### Implementation Notes

1. **Mutual Exclusivity**: When `--code` is provided, `patterns` should be empty or ignored
2. **Rule Requirement**: When using `--code`, at least one `--rule` should be specified (otherwise checking inline code against all rules is wasteful)
3. **Language Detection**: Use `.rs` extension for temp files by default, or infer from context
4. **Output**: Keep check's normal output format (less verbose than test's diagnostic mode)
5. **Quiet Mode**: Preserve the quiet mode behavior for testing without LLM execution

### Breaking Change Communication

This removes the `sah rule test` command. Users need to migrate:

```bash
# Old
sah rule test no-hardcoded-secrets --file test.rs
sah rule test no-hardcoded-secrets --code "fn main() {}"

# New
sah rule check test.rs --rule no-hardcoded-secrets
sah rule check --code "fn main() {}" --rule no-hardcoded-secrets
```



## Implementation Notes

All code review action items have been addressed:

### Changes Made

1. **Added `--code` example to doc comment** (check.rs:177)
   - Added example: `sah rule check --code "fn main() {}" --rule no-unwrap`

2. **Added validation for mutual exclusivity** (check.rs:180-185)
   - Error when both `--code` and file patterns are provided
   - Validation happens at the beginning of the function before any processing

3. **Added validation for --rule requirement** (check.rs:187-192)
   - Error when `--code` is provided without `--rule`
   - Uses `map_or(true, |r| r.is_empty())` to check for None or empty vec

4. **Added file extension to temp files** (check.rs:251-253)
   - Uses `tempfile::Builder::new().suffix(".rs")` for proper language detection

5. **Renamed variable for clarity** (check.rs:248)
   - Changed `_temp_file` to `_temp_file_guard` to make purpose clear

6. **Added comprehensive test coverage** (check.rs:680-750)
   - `test_execute_check_command_inline_code_requires_rule`: Tests --rule requirement
   - `test_execute_check_command_code_and_patterns_mutually_exclusive`: Tests mutual exclusivity

### Test Results

- All 16 check command tests pass
- All 1151 CLI tests pass
- Clippy passes with no warnings or errors

### Files Modified

- `swissarmyhammer-cli/src/commands/rule/check.rs`: Enhanced with all improvements
- All changes maintain backward compatibility for existing usage patterns
