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