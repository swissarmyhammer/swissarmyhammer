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