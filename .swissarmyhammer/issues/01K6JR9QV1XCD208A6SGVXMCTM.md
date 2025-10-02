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
