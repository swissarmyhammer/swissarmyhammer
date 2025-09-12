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

## Available Prompt Sources

Prompts are automatically loaded from all available sources:
- Built-in prompts (shipped with the tool)
- User prompts (~/.swissarmyhammer/prompts/)
- Local prompts (./.swissarmyhammer/prompts/)

## Architecture Changes

The prompt commands have been simplified:
- Removed complex source and category filtering from list command
- Moved output formatting controls to global arguments
- Streamlined parameter collection for test command
- Improved error handling and user feedback

All existing functionality is preserved while providing a cleaner, more consistent interface.