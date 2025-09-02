# Step 4: Register Test Command in CLI

Refer to /Users/wballard/github/sah/ideas/test.md

## Objective
Register the new test command in the CLI system by updating the main dispatcher and module exports.

## Task Details

### Update Commands Module (`commands/mod.rs`)
Add test module to the module exports:
```rust
pub mod test;  // Add this line
```

### Update Main CLI Dispatcher (`main.rs`)
Add test command to the main dispatcher around line 232:

```rust
Some(("test", _sub_matches)) => handle_test_command(&template_context).await,
```

### Create Handler Function in `main.rs`  
Add the test command handler function:
```rust
async fn handle_test_command(template_context: &TemplateContext) -> i32 {
    commands::test::handle_command(template_context).await
}
```

### Files to Modify
1. **`swissarmyhammer-cli/src/commands/mod.rs`** - Add module export
2. **`swissarmyhammer-cli/src/main.rs`** - Add command handler and dispatcher entry

## Implementation Location
Follow the exact pattern of the implement command:
- **Module export**: Line 10 in `commands/mod.rs` 
- **Dispatcher**: Around line 232 in `main.rs`
- **Handler function**: Around line 694 in `main.rs`

## Pattern Consistency
The registration follows the exact same pattern as:
- `implement` command (lines 232, 694-696)
- `plan` command  
- `flow` command
- All other top-level commands

## Validation Steps
1. **Compilation**: Code compiles without errors or warnings
2. **Help text**: `sah --help` shows test command in list
3. **Command recognition**: `sah test --help` works (may show workflow help)
4. **Integration**: Command integrates with existing CLI architecture

## Size Estimate
~10 lines of changes across 2 files

## Dependencies
- Step 3 (command implementation) must be complete
- Need working `commands::test::handle_command` function