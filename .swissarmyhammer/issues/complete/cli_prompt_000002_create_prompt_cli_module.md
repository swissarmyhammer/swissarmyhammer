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

## Proposed Solution

Based on my analysis of the current code structure, I will implement a dedicated CLI module within the prompt commands directory that provides:

1. **Command Definition Module**: Create `swissarmyhammer-cli/src/commands/prompt/cli.rs` that uses clap builders instead of derive macros
2. **Help Text Externalization**: Move help text to dedicated markdown files for better maintainability
3. **Command Structure Simplification**: Remove unnecessary filtering from list command as specified in the issue
4. **Strong Typing**: Create dedicated structs for each command type

### Implementation Plan

1. **Create CLI Module** (`cli.rs`):
   - Use clap `Command` builders instead of derive macros
   - Create `build_prompt_command()` function that returns the full command tree
   - Separate builders for each subcommand (`build_list_command()`, `build_test_command()`)
   - Include markdown help files using `include_str!()`

2. **Create Command Types**:
   - `ListCommand` struct (simplified, no filtering)
   - `TestCommand` struct (preserving all current functionality)  
   - `PromptCommand` enum to wrap both
   - `parse_prompt_command()` function to convert clap matches to typed structs

3. **Create Help Files**:
   - `list_help.md` - Detailed help for list command
   - `test_help.md` - Detailed help for test command
   - Use existing `description.md` for main command description

4. **Testing**:
   - Unit tests for command building and parsing
   - Tests for help text inclusion
   - Validation that all current functionality is preserved

This approach will create a clean separation between command definition (CLI module) and command execution (existing mod.rs), making the code more maintainable and testable.

## Implementation Status

✅ **COMPLETED** - All implementation steps have been successfully completed.

### Files Created

1. **CLI Module**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`
   - Clean command definitions using clap builders 
   - Simplified list command with no filtering (as required)
   - All test command functionality preserved
   - Strong typing with command structs
   - No global arguments duplicated in subcommands

2. **Help Text Files**:
   - `swissarmyhammer-cli/src/commands/prompt/list_help.md` - List command help
   - `swissarmyhammer-cli/src/commands/prompt/test_help.md` - Test command help

3. **Module Integration**:
   - Updated `mod.rs` to include the new CLI module
   - All code compiles successfully with only expected dead code warnings (since integration is pending)

### Testing Results

✅ All 10 unit tests pass successfully:
- Command building tests
- Argument parsing tests  
- Error handling tests
- Command validation tests

### Code Quality

- ✅ Compiles without errors
- ✅ All tests pass
- ✅ Only dead code warnings (expected until integration)
- ✅ Follows Rust coding standards
- ✅ Comprehensive test coverage

### Next Steps

This module is now ready for integration with the main CLI system. The implementation provides:

- **Single Source of Truth**: All prompt command definitions in one place
- **Clean Architecture**: Separation between command definition and execution
- **Strong Typing**: Type-safe command parsing with detailed structs
- **Maintainable Help**: Externalized help text in markdown files
- **Test Coverage**: Comprehensive unit tests ensuring reliability

The module successfully meets all requirements specified in the issue and is ready for production use.