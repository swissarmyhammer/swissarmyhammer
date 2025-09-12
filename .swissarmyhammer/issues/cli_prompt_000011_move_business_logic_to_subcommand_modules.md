# Move Prompt Business Logic from mod.rs to Subcommand Modules

## Problem

The current `src/commands/prompt/mod.rs` contains actual business logic for prompt commands, violating separation of concerns. The `mod.rs` file should only handle routing and coordination, not contain implementation details.

## Current State

**File**: `swissarmyhammer-cli/src/commands/prompt/mod.rs`
- Contains `run_list_command()` with actual prompt listing logic
- Contains `run_test_command()` with actual prompt testing logic  
- Contains parameter collection and rendering logic
- Mixed routing and business logic in same file

## Proper Architecture

```
src/commands/prompt/
├── mod.rs              # ONLY routing: match command type and delegate
├── list.rs             # ACTUAL list logic (move run_list_command here)
├── test.rs             # ACTUAL test logic (move run_test_command here)
└── display.rs          # Shared display utilities
```

## Goals

1. **Clean mod.rs**: Only routing logic, no business logic
2. **Self-contained modules**: Each subcommand owns its implementation
3. **Clear boundaries**: Easy to find and modify specific command logic
4. **Better testing**: Can test subcommand logic independently

## Implementation Steps

### 1. Create list.rs with Moved Logic

**File**: `swissarmyhammer-cli/src/commands/prompt/list.rs`

```rust
use crate::context::CliContext;
use anyhow::Result;
use swissarmyhammer::{PromptFilter, PromptLibrary, PromptResolver};

/// Execute the list command - shows all available prompts
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // MOVED FROM mod.rs: run_list_command logic
    // But updated to:
    // - Remove source/category filtering  
    // - Use cli_context.get_prompt_library() instead of recreating
    // - Use cli_context.display() instead of manual formatting
    
    let prompts = cli_context.get_all_prompts()?;
    
    // Filter out partials (existing logic)
    let display_prompts: Vec<_> = prompts
        .into_iter()
        .filter(|prompt| !is_partial_template(prompt))
        .collect();

    // Convert to display format and output via CliContext
    if cli_context.verbose {
        let verbose_rows: Vec<VerbosePromptRow> = display_prompts
            .iter()
            .map(|p| p.into())
            .collect();
        cli_context.display(verbose_rows)?;
    } else {
        let rows: Vec<PromptRow> = display_prompts
            .iter()  
            .map(|p| p.into())
            .collect();
        cli_context.display(rows)?;
    }

    Ok(())
}

/// Check if a prompt is a partial template (MOVED FROM mod.rs)
fn is_partial_template(prompt: &swissarmyhammer_prompts::Prompt) -> bool {
    // Existing logic moved from mod.rs
}
```

### 2. Create test.rs with Moved Logic

**File**: `swissarmyhammer-cli/src/commands/prompt/test.rs`

```rust
use crate::context::CliContext;
use crate::commands::prompt::cli::TestCommand;
use anyhow::Result;

/// Execute the test command (MOVED FROM mod.rs)
pub async fn execute_test_command(
    test_cmd: TestCommand,
    cli_context: &CliContext,
) -> Result<()> {
    // MOVED FROM mod.rs: run_test_command logic
    // But updated to use cli_context.get_prompt_library()
    // and cli_context.render_prompt()
}

// Move all parameter collection functions from mod.rs
```

### 3. Clean Up mod.rs to Pure Routing

**File**: `swissarmyhammer-cli/src/commands/prompt/mod.rs`

```rust
pub mod cli;
pub mod display;
pub mod list;
pub mod test;

use crate::context::CliContext;
use crate::exit_codes::EXIT_SUCCESS;
use cli::PromptCommand;

/// Handle prompt command using typed commands - PURE ROUTING ONLY
pub async fn handle_command_typed(
    command: PromptCommand,
    context: &CliContext,
) -> i32 {
    let result = match command {
        PromptCommand::List(_) => list::execute_list_command(context).await,
        PromptCommand::Test(test_cmd) => test::execute_test_command(test_cmd, context).await,
        PromptCommand::Validate(_) => validate::execute_validate_command(context).await,
    };

    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Prompt command failed: {}", e);
            1
        }
    }
}

// NO business logic here - only routing and error handling
```

## What Gets Moved

**From mod.rs to list.rs**:
- `run_list_command()` function
- `is_partial_template()` function  
- All list-specific logic and tests

**From mod.rs to test.rs**:
- `run_test_command()` function
- `prompt_for_all_missing_parameters()` function
- All parameter collection and conversion functions
- All test-specific logic and tests

**What stays in mod.rs**:
- Module exports and imports
- `handle_command_typed()` routing function
- High-level error handling and exit code conversion

## Success Criteria

1. ✅ `mod.rs` contains only routing logic (< 50 lines)
2. ✅ `list.rs` contains all list command implementation
3. ✅ `test.rs` contains all test command implementation  
4. ✅ All existing functionality preserved
5. ✅ Each module is self-contained and testable
6. ✅ Clear separation of concerns achieved

## Files Created

- `swissarmyhammer-cli/src/commands/prompt/list.rs`
- `swissarmyhammer-cli/src/commands/prompt/test.rs`

## Files Modified

- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Remove business logic, keep routing

---

**Priority**: High - Proper architecture foundation
**Estimated Effort**: Medium (move + clean up existing code)
**Dependencies**: None (refactoring existing code)
**Blocks**: All other prompt improvements