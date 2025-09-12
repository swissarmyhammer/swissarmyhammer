# Fix Prompt Command Architecture

## Problem

The prompt command implementation has overlapping systems and broken integration that needs to be fixed.

## Issues to Fix

1. **Duplicate parsing functions** - eliminate `parse_prompt_command_from_args`, use only `parse_prompt_command`
2. **Manual string parsing in main.rs** - use proper clap subcommand parsing
3. **Duplicate validate commands** - remove `sah prompt validate`, keep `sah validate`
4. **Business logic in mod.rs** - move to dedicated subcommand modules
5. **Recreating expensive objects** - use CliContext for prompt library access

## Fix Steps

### 1. Fix main.rs CLI Integration
- Update main.rs to use proper clap subcommand parsing for prompt
- Remove manual string argument extraction
- Use `parse_prompt_command(&matches)` instead of string parsing

### 2. Remove Duplicate Functions
- Delete `parse_prompt_command_from_args()` from cli.rs
- Update any remaining callers to use `parse_prompt_command(&matches)`

### 3. Remove Duplicate Validate Command
- Remove `PromptCommand::Validate` from cli.rs
- Remove validate handling from mod.rs
- Point users to `sah validate` for validation needs

### 4. Move Business Logic to Correct Modules
- Move `run_list_command()` from mod.rs to list.rs
- Move `run_test_command()` from mod.rs to test.rs
- Keep only routing logic in mod.rs

### 5. Fix CliContext Integration
- Update list/test handlers to use `cli_context.get_prompt_library()`
- Remove manual creation of PromptLibrary/PromptResolver
- Add display methods to CliContext

## Expected Result

- `cargo run -- prompt list` works
- `cargo run -- prompt test --help` works  
- Clean separation between routing and business logic
- No duplicate functions or commands
- Proper clap integration throughout

---

**Priority**: Critical
**Estimated Effort**: Medium
**Dependencies**: None