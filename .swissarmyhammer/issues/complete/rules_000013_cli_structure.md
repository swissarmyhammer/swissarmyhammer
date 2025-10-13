# Create CLI Rule Module Structure

Refer to ideas/rules.md

## Goal

Create the CLI command structure for rule commands, copying pattern from prompt commands.

## Context

The CLI needs a new `rule` subcommand with list/validate/check/test operations.

## Implementation

1. Create `swissarmyhammer-cli/src/commands/rule/` directory
2. Create module files copying pattern from `commands/prompt/`:
   - `mod.rs` - Module exports
   - `cli.rs` - Command definitions
   - `display.rs` - Display types
   - `list.rs` - List command
   - `validate.rs` - Validate command
   - `check.rs` - Check command
   - `test.rs` - Test command

3. Define `RuleCommand` enum in `cli.rs`:
```rust
pub enum RuleCommand {
    List(ListArgs),
    Validate(ValidateArgs),
    Check(CheckArgs),
    Test(TestArgs),
}
```

4. Wire up in `commands/mod.rs`

## Testing

- Verify module structure compiles
- Basic CLI parsing test

## Success Criteria

- [ ] Directory structure created
- [ ] Module files exist
- [ ] RuleCommand enum defined
- [ ] Wired into main CLI
- [ ] Compiles successfully



## Proposed Solution

After examining the prompt command structure, I will implement the rule CLI module following these steps:

1. **Directory Structure**: Create `swissarmyhammer-cli/src/commands/rule/` with the following files:
   - `mod.rs` - Module exports and main handler function
   - `cli.rs` - Command definitions and parsing logic
   - `display.rs` - Display types for list output
   - `list.rs` - List command implementation
   - `validate.rs` - Validate command implementation
   - `check.rs` - Check command implementation
   - `test.rs` - Test command implementation

2. **Command Structure**: Define `RuleCommand` enum in `cli.rs` with:
   - `List(ListCommand)` - List available rules
   - `Validate(ValidateCommand)` - Validate rule syntax
   - `Check(CheckCommand)` - Check code against rules
   - `Test(TestCommand)` - Test rules

3. **Pattern Consistency**: Follow the same patterns as `commands/prompt`:
   - Use `handle_command_typed()` and `run_rule_command_typed()` functions
   - Implement `parse_rule_command()` for clap ArgMatches parsing
   - Create display types with `Tabled` and `Serialize` derives
   - Include comprehensive unit tests

4. **Integration**: Wire into main CLI by adding `pub mod rule;` to `commands/mod.rs`

5. **Testing Strategy**: Use TDD to verify:
   - Module structure compiles
   - Command parsing works correctly
   - Each command can be instantiated
   - Error handling works as expected




## Implementation Notes

### Completed Tasks
1. ✅ Created directory structure: `swissarmyhammer-cli/src/commands/rule/`
2. ✅ Implemented all module files:
   - `mod.rs` - Handler functions with comprehensive tests
   - `cli.rs` - Command definitions and parsing with extensive tests
   - `display.rs` - Display types with Tabled/Serialize support and tests
   - `list.rs` - List command implementation with tests
   - `validate.rs` - Validate command stub with tests
   - `check.rs` - Check command stub with tests
   - `test.rs` - Test command stub with tests
3. ✅ Wired into `commands/mod.rs`
4. ✅ Added `swissarmyhammer-rules` dependency to Cargo.toml
5. ✅ Added `display_rules()` method to CliContext
6. ✅ All code compiles successfully
7. ✅ All tests pass (3136 tests passed)

### Key Decisions
- **Rule fields**: Used `metadata` for title (with fallback to name), and `category` for language display
- **Source tracking**: Implemented empty HashMap placeholder for sources until RuleResolver is available
- **Display pattern**: Followed exact pattern from prompt commands with emoji-based source display
- **Stub implementations**: Validate, check, and test commands print placeholder messages - ready for future implementation
- **Error handling**: Used consistent CliError pattern matching prompt module

### Files Modified
- `/Users/wballard/github/sah/swissarmyhammer-cli/Cargo.toml` - Added swissarmyhammer-rules dependency
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/mod.rs` - Added rule module
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/context.rs` - Added display_rules method

### Files Created
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/mod.rs`
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/cli.rs`
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/display.rs`
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/list.rs`
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/validate.rs`
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/check.rs`
- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/test.rs`

