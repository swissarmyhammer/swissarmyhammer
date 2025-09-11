# Create Simplified List Command Handler

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Create a new list command handler that is dramatically simplified compared to the current implementation. Remove source and category filtering, focus on clean display of all available prompts using the new display objects and CliContext pattern.

## Current State

- Complex `run_list_command()` with unnecessary filtering options
- Manual output formatting for different modes
- Over-complicated interface with source/category filters

## Goals

- Simplified list command that just shows all prompts
- Clean separation of business logic from display logic
- Use CliContext for output formatting AND prompt library access
- Filter out partial templates automatically
- No source/category filtering complexity

## Implementation Steps

### 1. Create List Handler Module

**File**: `swissarmyhammer-cli/src/commands/prompt/list.rs`

```rust
use crate::context::CliContext;
use crate::commands::prompt::display::{prompts_to_display_rows, DisplayRows};
use anyhow::Result;

/// Execute the list command - shows all available prompts
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // Get prompts from CliContext (don't recreate library/resolver)
    let prompts = cli_context.get_all_prompts()?;

    // Convert to display format based on verbose flag from CliContext
    let display_rows = prompts_to_display_rows(prompts, cli_context.verbose);

    // Use CliContext to handle output formatting
    cli_context.display(display_rows)?;

    Ok(())
}

/// Public interface for list command - ready for integration
pub async fn handle_list_command(cli_context: &CliContext) -> Result<i32> {
    match execute_list_command(cli_context).await {
        Ok(_) => Ok(crate::exit_codes::EXIT_SUCCESS),
        Err(e) => {
            eprintln!("List command failed: {}", e);
            Ok(crate::exit_codes::EXIT_ERROR)
        }
    }
}
```

**Key Change**: Use `cli_context.get_all_prompts()` instead of manually creating `PromptLibrary` and `PromptResolver`. The CliContext should provide access to the prompt library.

### 2. Update CliContext to Provide Prompt Library Access

**File**: `swissarmyhammer-cli/src/context.rs`

```rust
impl CliContext {
    /// Get all available prompts (should be cached/reused)
    pub fn get_all_prompts(&self) -> Result<Vec<Prompt>> {
        // Implementation should reuse existing library/resolver
        // Don't recreate these expensive objects
    }
    
    /// Display items using the configured format (table/json/yaml)
    pub fn display<T>(&self, items: Vec<T>) -> Result<()> 
    where 
        T: Tabled + Serialize 
    {
        // Handle output based on self.format
    }
}
```

## Testing Requirements

### Unit Tests
- Test partial template filtering
- Test prompt loading from CliContext
- Test error handling for failed prompt access

### Integration Tests
- Test full list command execution
- Test output formatting with CliContext
- Test handling of empty prompt lists
- Test error scenarios

## Success Criteria

1. ✅ Simplified list logic with no filtering complexity
2. ✅ Clean separation of loading, filtering, and display logic
3. ✅ Use CliContext for prompt library access AND output formatting
4. ✅ No recreation of expensive PromptLibrary/PromptResolver objects
5. ✅ Automatic filtering of partial templates
6. ✅ Comprehensive unit test coverage
7. ✅ Ready for integration with main command router

## Files Created

- `swissarmyhammer-cli/src/commands/prompt/list.rs` - List command handler

## Files Modified

- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Export list module
- `swissarmyhammer-cli/src/context.rs` - Add prompt library access methods

## Risk Mitigation

- Keep existing implementation until new one is integrated
- Comprehensive tests to validate filtering logic
- Error handling for all failure modes

---

**Estimated Effort**: Medium (200-300 lines including tests)
**Dependencies**: cli_prompt_000003_create_display_objects, cli_prompt_000001_add_global_format_argument
**Blocks**: cli_prompt_000006_update_main_command_routing