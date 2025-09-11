# Update Main Command Routing to Use New Prompt Architecture

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Update the main command routing in `main.rs` to use the new prompt command architecture. This switches prompt commands from the legacy static enum system to the new CliContext-based system while leaving all other commands unchanged.

## Current State

- `handle_prompt_command()` uses manual parsing of `PromptSubcommand` enum  
- Passes only `TemplateContext` instead of `CliContext`
- Complex manual argument parsing for each subcommand

## Goals

- Route prompt commands through new command handlers
- Use CliContext with global arguments for prompt commands only
- Clean up legacy prompt command routing code
- Preserve backward compatibility for all functionality
- Leave other commands unchanged during this transition

## Implementation Steps

### 1. Update Prompt Command Handler

**File**: `swissarmyhammer-cli/src/main.rs`

Replace the existing `handle_prompt_command()` function:

```rust
async fn handle_prompt_command(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,  // Changed from TemplateContext
) -> i32 {
    use crate::commands::prompt::cli::parse_prompt_command;
    
    // Parse prompt command using new parser
    let prompt_command = match parse_prompt_command(matches) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("Failed to parse prompt command: {}", e);
            return EXIT_ERROR;
        }
    };

    // Route to appropriate handler based on command type
    match prompt_command {
        crate::commands::prompt::cli::PromptCommand::List(_list_cmd) => {
            match crate::commands::prompt::list::handle_list_command(cli_context).await {
                Ok(exit_code) => exit_code,
                Err(e) => {
                    eprintln!("List command failed: {}", e);
                    EXIT_ERROR
                }
            }
        },
        crate::commands::prompt::cli::PromptCommand::Test(test_cmd) => {
            match crate::commands::prompt::test::handle_test_command(test_cmd, cli_context).await {
                Ok(exit_code) => exit_code,
                Err(e) => {
                    eprintln!("Test command failed: {}", e);
                    EXIT_ERROR
                }
            }
        }
    }
}
```

### 2. Update Command Router Call

**File**: `swissarmyhammer-cli/src/main.rs`

In `handle_dynamic_matches()`, update the prompt command call:

```rust
Some(("prompt", sub_matches)) => {
    // Pass CliContext instead of TemplateContext
    handle_prompt_command(sub_matches, &context).await
}
```

### 3. Update CliContext to Extract Prompt-Specific Global Args

**File**: `swissarmyhammer-cli/src/context.rs`

Ensure CliContext can extract prompt-relevant global arguments:

```rust
impl CliContext {
    pub async fn new(
        template_context: TemplateContext,
        matches: clap::ArgMatches,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Extract global arguments that affect prompt commands
        let format = match matches.get_one::<String>("format").map(|s| s.as_str()) {
            Some("json") => crate::cli::OutputFormat::Json,
            Some("yaml") => crate::cli::OutputFormat::Yaml,
            _ => crate::cli::OutputFormat::Table,
        };
        
        let verbose = matches.get_flag("verbose");
        let debug = matches.get_flag("debug");
        let quiet = matches.get_flag("quiet");

        Ok(Self {
            template_context,
            format,
            verbose,
            debug,
            quiet,
            matches,
        })
    }
}
```

### 4. Remove Legacy Prompt Command Parsing

**File**: `swissarmyhammer-cli/src/main.rs`

Remove the old complex manual parsing from the existing `handle_prompt_command()`:

```rust
// Remove this entire section:
/*
let subcommand = match matches.subcommand() {
    Some(("list", sub_matches)) => {
        let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
            Some("json") => OutputFormat::Json,
            Some("yaml") => OutputFormat::Yaml,
            _ => OutputFormat::Table,
        };
        // ... rest of complex manual parsing
    }
    // ... other manual parsing
};
*/
```

### 5. Update Error Handling

**File**: `swissarmyhammer-cli/src/main.rs`

Ensure consistent error handling for the new prompt command flow:

```rust
async fn handle_prompt_command(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
) -> i32 {
    // Log command execution in debug mode
    if cli_context.debug {
        println!("Executing prompt command with global args: verbose={}, format={:?}", 
            cli_context.verbose, cli_context.format);
    }

    // Parse and route the command
    let prompt_command = match crate::commands::prompt::cli::parse_prompt_command(matches) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("Error: {}", e);
            if cli_context.debug {
                eprintln!("Debug: Failed to parse prompt command: {:#?}", e);
            }
            return EXIT_ERROR;
        }
    };

    // Execute the appropriate handler
    match prompt_command {
        crate::commands::prompt::cli::PromptCommand::List(_) => {
            if cli_context.debug {
                println!("Executing list command");
            }
            crate::commands::prompt::list::handle_list_command(cli_context).await
                .unwrap_or_else(|e| {
                    eprintln!("List command error: {}", e);
                    EXIT_ERROR
                })
        },
        crate::commands::prompt::cli::PromptCommand::Test(test_cmd) => {
            if cli_context.debug {
                println!("Executing test command for prompt: {:?}", test_cmd.prompt_name);
            }
            crate::commands::prompt::test::handle_test_command(test_cmd, cli_context).await
                .unwrap_or_else(|e| {
                    eprintln!("Test command error: {}", e);
                    EXIT_ERROR
                })
        }
    }
}
```

## Testing Requirements

### Integration Tests
- Test `sah --verbose prompt list` works correctly
- Test `sah --format=json prompt list` outputs JSON
- Test `sah --debug prompt test help` shows debug information
- Test error handling for invalid prompt commands
- Test that other commands still work unchanged

### Regression Tests
- Test all existing prompt functionality still works
- Test backward compatibility with current prompt usage
- Test error messages are user-friendly

## Success Criteria

1. ✅ Prompt commands use new architecture with CliContext
2. ✅ Global arguments (--verbose, --format, --debug) work with prompt commands
3. ✅ All existing prompt functionality preserved
4. ✅ Clean error handling and user feedback
5. ✅ Other commands remain unchanged and functional
6. ✅ No breaking changes to existing prompt command usage

## Files Modified

- `swissarmyhammer-cli/src/main.rs` - Update prompt command routing
- `swissarmyhammer-cli/src/context.rs` - Enhance CliContext global arg extraction

## Risk Mitigation

- Comprehensive integration testing before switching
- Keep legacy code temporarily for rollback if needed
- Test all prompt command variations thoroughly
- Validate error handling in all failure scenarios

## Validation Commands

Test these specific scenarios to ensure success:

```bash
# Basic functionality
sah prompt list
sah prompt test help

# Global arguments  
sah --verbose prompt list
sah --format=json prompt list
sah --format=yaml prompt list
sah --debug prompt test help --var topic=git

# Error scenarios
sah prompt invalid-subcommand
sah prompt test non-existent-prompt
sah prompt test help --var invalid_format

# Ensure other commands unchanged
sah doctor
sah flow list
sah serve --help
```

---

**Estimated Effort**: Medium (150-250 lines changed)
**Dependencies**: cli_prompt_000004_create_list_handler, cli_prompt_000005_create_test_handler
**Blocks**: cli_prompt_000007_remove_legacy_code