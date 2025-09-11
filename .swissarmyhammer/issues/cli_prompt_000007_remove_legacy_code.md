# Remove Legacy Prompt Command Code

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Remove the legacy prompt command implementation after confirming the new architecture works correctly. This includes removing the `PromptSubcommand` enum, cleaning up unused imports, and removing the old command handler code.

## Current State

- `PromptSubcommand` enum still exists in `src/cli.rs`
- Legacy command handler code in `src/commands/prompt/mod.rs`
- Unused imports and dependencies

## Goals

- Remove duplicate command definitions
- Clean up unused code and imports
- Ensure no references to legacy prompt structures remain
- Maintain clean codebase with single source of truth

## Implementation Steps

### 1. Remove PromptSubcommand Enum

**File**: `swissarmyhammer-cli/src/cli.rs`

Remove the entire `PromptSubcommand` enum and related code:

```rust
// Remove this entire enum:
/*
#[derive(Subcommand, Debug)]
pub enum PromptSubcommand {
    List {
        format: OutputFormat,
        verbose: bool,
        source: Option<PromptSourceArg>,
        category: Option<String>,
    },
    Test {
        prompt_name: Option<String>,
        file: Option<String>,
        vars: Vec<String>,
        raw: bool,
        copy: bool,
        save: Option<String>,
        debug: bool,
    },
}
*/
```

### 2. Update Commands Enum

**File**: `swissarmyhammer-cli/src/cli.rs`

Update the main Commands enum to remove the complex prompt subcommand:

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ... other commands remain unchanged
    
    /// Manage and test prompts
    #[command(long_about = commands::prompt::DESCRIPTION)]
    Prompt {
        // Remove this subcommand field - prompt routing handled dynamically
        #[command(subcommand)]
        subcommand: PromptSubcommand,  // <- REMOVE THIS LINE
    },
    
    // ... other commands
}
```

Actually, we need to handle this carefully. The Commands enum still needs a Prompt variant, but we'll route it through the dynamic system. Let's modify this:

```rust
/// Manage and test prompts
#[command(long_about = commands::prompt::DESCRIPTION)]
Prompt {
    // Prompt subcommands now handled by the dynamic CLI system
    // Remove the subcommand field - routing handled in main.rs
},
```

### 3. Remove Legacy Handler Code

**File**: `swissarmyhammer-cli/src/commands/prompt/mod.rs`

Remove or significantly simplify the legacy handler code:

```rust
//! Prompt command implementation - modernized
//!
//! Uses the new CliContext pattern with clean command separation

// Keep the DESCRIPTION constant for the CLI
pub const DESCRIPTION: &str = include_str!("description.md");

// Remove the old handle_command function:
/*
pub async fn handle_command(
    subcommand: PromptSubcommand,
    template_context: &TemplateContext,
) -> i32 {
    // ... old implementation
}
*/

// Remove all the old implementation functions:
/*
async fn run_prompt_command(...) -> CliResult<()> { ... }
fn run_list_command(...) -> Result<(), anyhow::Error> { ... }
async fn run_test_command(...) -> Result<(), anyhow::Error> { ... }
// etc.
*/

// Keep modules that are still used
pub mod cli;
pub mod display;
pub mod list;
pub mod test;
```

### 4. Clean Up Imports

**File**: `swissarmyhammer-cli/src/cli.rs`

Remove unused imports related to prompt commands:

```rust
// Remove these if no longer used:
/*
use crate::cli::PromptSubcommand;
use swissarmyhammer::PromptFilter;
// etc.
*/
```

**File**: `swissarmyhammer-cli/src/main.rs`

Update imports to remove legacy prompt types:

```rust
// Remove:
/*
use crate::cli::{OutputFormat, PromptSourceArg, PromptSubcommand};
*/

// Keep only what's needed for the new system
use crate::context::CliContext;
```

### 5. Update Tests

**File**: `swissarmyhammer-cli/src/cli.rs` (tests section)

Remove or update tests that reference the old `PromptSubcommand`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Remove tests like:
    /*
    #[test]
    fn test_cli_prompt_list_subcommand() {
        // ... old test code
    }
    */

    // Keep tests that are still relevant to the root CLI structure
    // Update tests to use the new dynamic prompt routing
}
```

### 6. Verify Dynamic CLI Integration

**File**: `swissarmyhammer-cli/src/dynamic_cli.rs`

Ensure the dynamic CLI system properly handles prompt commands now that the static enum is removed. This may require updates to route prompt commands through the new system.

### 7. Update Documentation

**File**: `swissarmyhammer-cli/src/commands/prompt/description.md`

Update the help text to reflect the simplified architecture:

```markdown
# Prompt Commands

Manage and test prompts with simplified, clean commands.

## List Command
`sah prompt list` - Shows all available prompts

Use global flags to control output:
- `sah --verbose prompt list` - Show detailed information
- `sah --format=json prompt list` - Output as JSON
- `sah --format=yaml prompt list` - Output as YAML

## Test Command  
`sah prompt test <name>` - Test prompts interactively

Examples:
- `sah prompt test help`
- `sah prompt test code-review --var author=John`
- `sah --debug prompt test plan`

The prompt system has been simplified to focus on core functionality
without unnecessary filtering complexity.
```

## Testing Requirements

### Compilation Tests
- Ensure code compiles without errors after removals
- Check that all imports resolve correctly
- Verify no unused code warnings

### Integration Tests
- Test that prompt commands still work through new system
- Verify error messages are appropriate
- Test help text displays correctly

### Regression Tests
- Ensure all existing prompt functionality works
- Test edge cases and error scenarios
- Verify other commands remain unaffected

## Success Criteria

1. ✅ No duplicate prompt command definitions
2. ✅ Clean codebase with no unused legacy code
3. ✅ All tests pass with new architecture
4. ✅ No compiler warnings about unused code
5. ✅ Prompt commands work identically to before cleanup
6. ✅ Help text reflects simplified architecture

## Files Modified

- `swissarmyhammer-cli/src/cli.rs` - Remove PromptSubcommand enum and related code
- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Remove legacy handler implementation  
- `swissarmyhammer-cli/src/main.rs` - Clean up unused imports
- `swissarmyhammer-cli/src/commands/prompt/description.md` - Update help text

## Risk Mitigation

- Thorough testing before and after removal
- Commit changes incrementally for easy rollback
- Keep commented backup of removed code temporarily
- Test all prompt command variations

## Validation

After this step, these should all work identically to before:

```bash
# Basic commands
sah prompt list
sah prompt test help

# With global arguments
sah --verbose prompt list  
sah --format=json prompt list
sah --debug prompt test help --var topic=git

# Error cases
sah prompt nonexistent-command
sah prompt test nonexistent-prompt
```

And verify clean compilation:
```bash
cargo build
cargo clippy
cargo test
```

---

**Estimated Effort**: Small (< 100 lines removed, minimal additions)
**Dependencies**: cli_prompt_000006_update_main_command_routing
**Blocks**: cli_prompt_000008_comprehensive_testing