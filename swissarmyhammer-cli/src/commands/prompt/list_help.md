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