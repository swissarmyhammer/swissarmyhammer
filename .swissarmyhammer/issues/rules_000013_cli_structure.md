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
