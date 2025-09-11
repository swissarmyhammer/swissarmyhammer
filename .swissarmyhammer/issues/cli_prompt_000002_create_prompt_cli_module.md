# Create Prompt CLI Command Definition Module

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Create a new `cli.rs` module within `commands/prompt/` that serves as the single source of truth for prompt command definitions. This module will use clap builders and exclude global arguments (which are handled at the root level).

## Current State

- Prompt commands defined in `PromptSubcommand` enum in root `cli.rs`
- Manual argument parsing scattered across files
- Over-complex list command with unnecessary filtering options

## Goals

- Single source of truth for prompt command definitions
- Clean command structure using clap builders
- Simplified list command (no source/category filtering)
- Help text sourced from markdown files

## Implementation Steps

### 1. Create CLI Module

**File**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`

```rust
use clap::{Arg, Command};

/// Build the prompt command with all subcommands
pub fn build_prompt_command() -> Command {
    Command::new("prompt")
        .about(include_str!("description.md"))
        .subcommand(build_list_command())
        .subcommand(build_test_command())
}

/// Build the list subcommand - simplified with no filtering
fn build_list_command() -> Command {
    Command::new("list")
        .about("List all available prompts")
        .long_about(include_str!("list_help.md"))
        // No --format, --verbose here - handled globally
}

/// Build the test subcommand with necessary arguments only
fn build_test_command() -> Command {
    Command::new("test")
        .about("Test prompts interactively with sample arguments") 
        .long_about(include_str!("test_help.md"))
        .arg(
            Arg::new("prompt_name")
                .help("Prompt name to test")
                .value_name("PROMPT_NAME")
        )
        .arg(
            Arg::new("file")
                .short('f')
                .long("file") 
                .help("Path to prompt file to test")
                .value_name("FILE")
        )
        .arg(
            Arg::new("vars")
                .long("var")
                .alias("arg")
                .help("Variables as key=value pairs")
                .value_name("KEY=VALUE")
                .action(clap::ArgAction::Append)
        )
        // Keep other existing test flags
}
```

### 2. Create Help Markdown Files

**File**: `swissarmyhammer-cli/src/commands/prompt/list_help.md`
```markdown
Lists all available prompts from all sources (built-in, user, local).
Shows prompt names, titles, and basic information.

The list command has been simplified to show all available prompts.
Use global --verbose and --format flags to control output detail and format.

Examples:
  sah prompt list                    # Show all prompts
  sah --verbose prompt list          # Show detailed information
  sah --format=json prompt list      # Output as JSON
```

**File**: `swissarmyhammer-cli/src/commands/prompt/test_help.md`
```markdown
Test prompts interactively to see how they render with different arguments.
Helps debug template errors and refine prompt content.

Usage modes:
  sah prompt test prompt-name                    # Interactive test
  sah prompt test -f path/to/prompt.md          # Test from file  
  sah prompt test prompt-name --var key=value   # Non-interactive

Examples:
  sah prompt test code-review
  sah prompt test help --var topic=git
  sah --verbose prompt test plan
```

### 3. Create Internal Command Types

**File**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`

```rust
#[derive(Debug)]
pub struct ListCommand {
    // Simplified - no filtering options
    // Uses global verbose/format from CliContext
}

#[derive(Debug)]
pub struct TestCommand {
    pub prompt_name: Option<String>,
    pub file: Option<String>, 
    pub vars: Vec<String>,
    pub raw: bool,
    pub copy: bool,
    pub save: Option<String>,
    pub debug: bool,
}

/// Parse clap matches into command structs
pub fn parse_prompt_command(matches: &clap::ArgMatches) -> Result<PromptCommand, ParseError> {
    match matches.subcommand() {
        Some(("list", _)) => Ok(PromptCommand::List(ListCommand {})),
        Some(("test", sub_matches)) => {
            let test_cmd = TestCommand {
                prompt_name: sub_matches.get_one::<String>("prompt_name").cloned(),
                file: sub_matches.get_one::<String>("file").cloned(),
                vars: sub_matches.get_many::<String>("vars")
                    .map(|vals| vals.cloned().collect())
                    .unwrap_or_default(),
                // ... other fields
            };
            Ok(PromptCommand::Test(test_cmd))
        },
        _ => Err(ParseError::UnknownSubcommand)
    }
}
```

### 4. Create Command Enum

**File**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`

```rust
#[derive(Debug)]
pub enum PromptCommand {
    List(ListCommand),
    Test(TestCommand), 
}
```

## Testing Requirements

### Unit Tests
- Test command parsing for all subcommands
- Test help text generation from markdown files
- Test argument validation and error handling

### Integration Tests
- Test that commands build without errors
- Test argument parsing edge cases
- Test help text display

## Success Criteria

1. ✅ Clean command definitions using clap builders
2. ✅ Simplified list command with no filtering
3. ✅ Help text sourced from markdown files
4. ✅ Strong typing with command structs
5. ✅ No global arguments duplicated in subcommands
6. ✅ All current test command functionality preserved

## Files Created

- `swissarmyhammer-cli/src/commands/prompt/cli.rs` - Command definitions
- `swissarmyhammer-cli/src/commands/prompt/list_help.md` - List help text
- `swissarmyhammer-cli/src/commands/prompt/test_help.md` - Test help text

## Risk Mitigation

- Keep existing implementation until new one is fully tested
- Comprehensive unit tests for all parsing logic
- Validate help text renders correctly

---

**Estimated Effort**: Medium (100-200 lines)
**Dependencies**: cli_prompt_000001_add_global_format_argument
**Blocks**: All subsequent implementation steps