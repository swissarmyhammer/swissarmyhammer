# Update Documentation and Help Text

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Update all documentation, help text, and user-facing materials to reflect the simplified prompt command architecture and new global argument patterns. This ensures users understand the new interface and can effectively use the cleaned-up commands.

## Current State

- Help text still references old complex filtering options
- Documentation may mention removed source/category filters
- Global argument usage not clearly documented

## Goals

- Clear, accurate help text reflecting simplified commands
- Updated documentation about global argument usage
- Consistent messaging across all help sources
- Examples that showcase the new simplified interface
- Migration guidance for users familiar with old interface

## Implementation Steps

### 1. Update Main Prompt Help Text

**File**: `swissarmyhammer-cli/src/commands/prompt/description.md`

```markdown
# Prompt Commands

Manage and test prompts with a clean, simplified interface.

The prompt system provides two main commands for working with prompts:
- `list` - Display all available prompts  
- `test` - Test prompts interactively with sample data

## Global Arguments

Use global arguments to control output and behavior:

- `--verbose` - Show detailed information and debug output
- `--format <FORMAT>` - Output format: table (default), json, yaml  
- `--debug` - Enable debug mode with detailed tracing
- `--quiet` - Suppress all output except errors

## Examples

### List Commands
```bash
# Basic usage
sah prompt list

# With global arguments
sah --verbose prompt list           # Show detailed prompt information
sah --format=json prompt list       # Output as JSON
sah --format=yaml prompt list       # Output as YAML
```

### Test Commands  
```bash
# Interactive testing
sah prompt test code-review         # Prompt for all parameters interactively
sah prompt test help                # Test the help prompt

# Non-interactive with parameters
sah prompt test help --var topic=git --var format=markdown
sah prompt test code-review --var author=John --var version=1.0

# With global arguments
sah --verbose prompt test plan      # Show detailed execution information  
sah --debug prompt test help        # Enable debug mode for troubleshooting
```

## Architecture Changes

The prompt commands have been simplified:
- Removed complex source and category filtering from list command
- Moved output formatting controls to global arguments
- Streamlined parameter collection for test command
- Improved error handling and user feedback

All existing functionality is preserved while providing a cleaner, more consistent interface.
```

### 2. Update List Command Help

**File**: `swissarmyhammer-cli/src/commands/prompt/list_help.md`

```markdown
# Prompt List Command

Display all available prompts from all sources (built-in, user, local).

## Usage
```
sah prompt list
```

## Global Options

Control output using global arguments:

```bash
sah --verbose prompt list           # Show detailed information including descriptions
sah --format=json prompt list       # Output as JSON for scripting
sah --format=yaml prompt list       # Output as YAML for scripting  
```

## Output

### Standard Output (default)
Shows prompt names and titles in a clean table format.

### Verbose Output (--verbose)
Shows additional information including:
- Full descriptions
- Source information (builtin, user, local)
- Categories and tags
- Parameter counts

### Structured Output (--format=json|yaml)
Machine-readable output suitable for scripting and automation.

## Examples

```bash
# Basic list
sah prompt list

# Detailed information  
sah --verbose prompt list

# JSON output for scripts
sah --format=json prompt list | jq '.[] | .name'

# Save YAML output
sah --format=yaml prompt list > prompts.yaml
```

## Notes

- Partial templates (internal templates used by other prompts) are automatically filtered out
- All available prompt sources are included automatically
- Use global `--quiet` to suppress output except errors
```

### 3. Update Test Command Help

**File**: `swissarmyhammer-cli/src/commands/prompt/test_help.md`

```markdown
# Prompt Test Command

Test prompts interactively to see how they render with different arguments.
Perfect for debugging template issues and previewing prompt output.

## Usage
```
sah prompt test <PROMPT_NAME> [OPTIONS]
sah prompt test --file <FILE> [OPTIONS]
```

## Arguments

- `<PROMPT_NAME>` - Name of the prompt to test
- `--file <FILE>` - Path to a local prompt file to test

## Options

- `--var <KEY=VALUE>` - Set template variables (can be used multiple times)
- `--raw` - Output raw prompt without additional formatting
- `--copy` - Copy rendered prompt to clipboard (if supported)
- `--save <FILE>` - Save rendered prompt to file
- `--debug` - Show debug information during processing

## Global Options

- `--verbose` - Show detailed execution information
- `--debug` - Enable comprehensive debug output
- `--quiet` - Suppress all output except the rendered prompt

## Interactive Mode

When variables are not provided via `--var`, the command prompts interactively:

- Shows parameter descriptions and default values
- Validates input according to parameter types
- Supports boolean (true/false, yes/no, 1/0), numbers, choices
- Detects non-interactive environments (CI/CD) and uses defaults

## Examples

### Basic Testing
```bash
# Interactive mode - prompts for all parameters
sah prompt test code-review

# Non-interactive with all parameters provided  
sah prompt test help --var topic=git --var format=markdown

# Test from file
sah prompt test --file ./my-prompt.md --var name=John
```

### Advanced Usage
```bash
# Verbose output with debug information
sah --verbose --debug prompt test plan --var project=myapp

# Save output to file
sah prompt test help --var topic=testing --save help-output.md

# Raw output (no extra formatting)
sah prompt test summary --var title="Project Status" --raw

# Multiple variables
sah prompt test code-review \
  --var author=Jane \
  --var version=2.1 \
  --var language=Python \
  --var files=src/main.py,tests/test_main.py
```

### Parameter Types

The test command supports various parameter types:

- **String**: Free-form text input
- **Boolean**: true/false, yes/no, 1/0, t/f, y/n
- **Number**: Integer or decimal values
- **Choice**: Select from predefined options  
- **MultiChoice**: Select multiple options (comma-separated)

### Non-Interactive Environments

In CI/CD or scripted environments:
- Uses default values for optional parameters
- Fails with clear error for required parameters without defaults
- Automatically detected based on terminal availability

## Error Handling

Clear error messages for common issues:
- Prompt not found
- Invalid parameter values
- Missing required parameters
- Template rendering errors
- File system errors (save/load operations)
```

### 4. Update CLI Help Text

**File**: `swissarmyhammer-cli/src/cli.rs`

Update the main CLI help text and command descriptions:

```rust
#[derive(Parser, Debug)]
#[command(name = "swissarmyhammer")]
#[command(version)]
#[command(about = "An MCP server for managing prompts as markdown files")]
#[command(long_about = "
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

Global arguments can be used with any command to control output and behavior:
  --verbose     Show detailed information and debug output
  --format      Set output format (table, json, yaml) for commands that support it  
  --debug       Enable debug mode with comprehensive tracing
  --quiet       Suppress all output except errors

Example usage:
  swissarmyhammer serve                           # Run as MCP server
  swissarmyhammer doctor                          # Check configuration
  swissarmyhammer --verbose prompt list          # List prompts with details
  swissarmyhammer --format=json prompt list      # List prompts as JSON
  swissarmyhammer --debug prompt test help       # Test prompt with debug info
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable verbose logging and detailed output
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable debug logging and comprehensive tracing
    #[arg(short, long)]
    pub debug: bool,

    /// Suppress all output except errors
    #[arg(short, long)]
    pub quiet: bool,
    
    /// Output format for commands that support it
    #[arg(long, value_enum, default_value = "table")]
    pub format: OutputFormat,
}
```

### 5. Update Command Description

```rust
/// Manage and test prompts
#[command(long_about = "
Manage and test prompts with a clean, simplified interface.

The prompt system provides two main commands:
• list - Display all available prompts from all sources  
• test - Test prompts interactively with sample data

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah prompt list                           # List all prompts
  sah --verbose prompt list                 # Show detailed information
  sah --format=json prompt list             # Output as JSON
  sah prompt test code-review               # Interactive testing
  sah prompt test help --var topic=git      # Test with parameters  
  sah --debug prompt test plan              # Test with debug output
")]
Prompt {
    // Subcommand routing handled dynamically
},
```

### 6. Add Migration Guide

**File**: `swissarmyhammer-cli/docs/prompt_command_migration.md`

```markdown
# Prompt Command Migration Guide

This guide helps users migrate from the old prompt command interface to the new simplified architecture.

## What Changed

### Removed Features
- `--source` filter from list command (now shows all sources automatically)
- `--category` filter from list command (now shows all prompts)  
- Per-command `--format` and `--verbose` flags

### Added Features  
- Global `--format` argument works with all commands
- Global `--verbose` and `--debug` arguments for consistent behavior
- Cleaner, more predictable output formatting
- Better error handling and user feedback

## Migration Examples

### List Command

**Old:**
```bash
sah prompt list --verbose --format=json --source=builtin
sah prompt list --category=dev --format=yaml
```

**New:**
```bash  
sah --verbose --format=json prompt list
sah --format=yaml prompt list
```

All prompts are now shown by default. To filter output, use standard tools:
```bash
sah --format=json prompt list | jq '.[] | select(.category == "dev")'
```

### Test Command

**Old and New (unchanged):**
```bash
sah prompt test code-review --var author=John
sah prompt test help --var topic=git --debug
```

Global arguments now work consistently:
```bash
sah --verbose prompt test help --var topic=git
sah --debug prompt test code-review --var author=John
```

## Benefits

- **Consistency**: Global arguments work the same across all commands
- **Simplicity**: Fewer command-specific flags to remember  
- **Predictability**: Standard behavior for output formatting
- **Discoverability**: Easier to understand available options

## Troubleshooting

### "Unknown argument --source"
The `--source` filter has been removed. All prompt sources are included automatically.

### "Unknown argument --category"  
The `--category` filter has been removed. Use external tools like `jq` to filter JSON output.

### "Format not working"
Use the global `--format` argument: `sah --format=json prompt list`

### "Verbose not showing details"
Use the global `--verbose` argument: `sah --verbose prompt list`
```

## Testing Requirements

### Documentation Tests
- Verify all help text displays correctly
- Test all example commands in documentation
- Validate help text matches actual command behavior

### User Experience Tests  
- Test help text accessibility and clarity
- Verify examples work as documented
- Test error messages are helpful

## Success Criteria

1. ✅ All help text accurately reflects new architecture
2. ✅ Examples in documentation work correctly
3. ✅ Global argument usage clearly explained
4. ✅ Migration guide helps existing users
5. ✅ Consistent terminology across all documentation
6. ✅ Clear, actionable error messages

## Files Created

- `swissarmyhammer-cli/docs/prompt_command_migration.md` - Migration guide for existing users

## Files Modified

- `swissarmyhammer-cli/src/commands/prompt/description.md` - Main prompt help
- `swissarmyhammer-cli/src/commands/prompt/list_help.md` - List command help  
- `swissarmyhammer-cli/src/commands/prompt/test_help.md` - Test command help
- `swissarmyhammer-cli/src/cli.rs` - CLI help text and descriptions

## Risk Mitigation

- Test all documented examples to ensure they work
- Get feedback on help text clarity and completeness
- Validate migration guide with real usage scenarios

---

**Estimated Effort**: Medium (200-300 lines of documentation)
**Dependencies**: cli_prompt_000008_comprehensive_testing
**Blocks**: cli_prompt_000010_final_validation

## Proposed Solution

Based on my analysis of the current CLI structure, I need to update the documentation to reflect the simplified prompt command architecture. Here's my implementation plan:

### Files to Update:

1. **CLI Help Text (`src/cli.rs`)**
   - Update main CLI long_about to document global arguments properly
   - Update Prompt command description to reflect simplified interface
   - Ensure examples show correct usage of global arguments

2. **Prompt Command Description (`src/commands/prompt/description.md`)**
   - Replace current basic description with comprehensive documentation
   - Add examples of global argument usage
   - Document the simplified list/test interface
   - Remove references to removed filtering options

3. **Create Missing Documentation Files**
   - `src/commands/prompt/list_help.md` - Detailed list command help
   - `src/commands/prompt/test_help.md` - Detailed test command help
   - `docs/prompt_command_migration.md` - Migration guide for users

### Key Changes:
- Emphasize global arguments (`--verbose`, `--format`, `--debug`, `--quiet`)
- Show that filtering is now done via external tools like `jq`
- Document the simplified command structure (list/test only)
- Provide clear migration examples from old to new syntax

### Implementation Strategy:
- Update existing files with new content
- Create new help files for detailed command documentation
- Add migration guide to help existing users transition
- Test all documented examples to ensure they work correctly
## Implementation Complete ✅

I have successfully updated all documentation and help text to reflect the simplified prompt command architecture. Here's what was accomplished:

### Files Updated:

1. **Main CLI Help Text (`src/cli.rs`)**
   - ✅ Updated main CLI long_about to document global arguments properly
   - ✅ Updated Prompt command description to reflect simplified interface
   - ✅ Added examples showing correct usage of global arguments

2. **Dynamic CLI System (`src/dynamic_cli.rs`)**
   - ✅ Removed --source and --category filters from list command
   - ✅ Updated both list and test commands with comprehensive long_about documentation
   - ✅ Added detailed usage examples and explanations

3. **Prompt Command Description (`src/commands/prompt/description.md`)**
   - ✅ Completely replaced with comprehensive markdown documentation
   - ✅ Added examples of global argument usage
   - ✅ Documented the simplified list/test interface

### Files Created:

4. **List Command Help (`src/commands/prompt/list_help.md`)**
   - ✅ Detailed help for the list command
   - ✅ Examples showing global argument usage
   - ✅ Clear explanation of output formats

5. **Test Command Help (`src/commands/prompt/test_help.md`)**
   - ✅ Comprehensive help for the test command
   - ✅ Interactive mode documentation
   - ✅ Advanced usage examples with multiple variables

6. **Migration Guide (`docs/prompt_command_migration.md`)**
   - ✅ Guide for users transitioning from old to new interface
   - ✅ Clear before/after examples
   - ✅ Troubleshooting section for common issues

### Key Improvements:
- **Consistency**: Global arguments now work the same across all commands
- **Simplicity**: Removed complex filtering options (--source, --category)
- **Clarity**: All examples show proper global argument usage
- **Completeness**: Comprehensive help text for all commands
- **Migration Support**: Clear guidance for existing users

### Testing Results:
- ✅ Clean build successful after cargo clean
- ✅ `prompt list --help` shows new documentation without old filters
- ✅ `prompt test --help` shows comprehensive usage documentation
- ✅ All filtering options successfully removed from CLI interface
- ✅ Global arguments (--verbose, --format, --debug) work as documented

The documentation now accurately reflects the simplified prompt command architecture and provides clear guidance for users on how to use the new global argument patterns.
# CLI Prompt Documentation Update

## Goal
Update documentation and help text to reflect the simplified prompt command architecture after removing the legacy PromptSubcommand enum.

## Changes Made
- Removed legacy PromptSubcommand enum
- Implemented modern CLI architecture with proper typed commands
- Added comprehensive help documentation files
- Updated command routing and handlers

## Code Review Results ✅

### Issues Found and Fixed
1. **Clippy Warning Fixed**: Resolved `clippy::empty_line_after_doc_comments` warning in `swissarmyhammer-cli/src/commands/prompt/mod.rs:18`
   - Combined separate doc comment blocks into a single coherent comment
   - Modified comment to properly document the function purpose

### Code Quality Assessment
- ✅ **Build**: Clean successful compilation
- ✅ **Clippy**: No warnings or errors after fix
- ✅ **Tests**: All tests passing
- ✅ **Documentation**: Comprehensive help text and user documentation added
- ✅ **Architecture**: Clean separation of concerns with proper module structure

### Summary
The code review identified and fixed one minor clippy warning. The codebase demonstrates:
- Excellent test coverage with comprehensive CLI parsing tests
- Strong type safety and proper error handling
- Clean architecture with proper separation of concerns
- Comprehensive documentation for user experience

The documentation update has been successfully completed with high code quality maintained throughout the implementation.

**Status**: ✅ Ready for merge after successful code review and fixes