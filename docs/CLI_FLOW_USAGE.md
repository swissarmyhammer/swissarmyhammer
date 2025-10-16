# Flow Command Usage Guide

## Overview

The flow system provides two ways to execute workflows:

1. **Shortcut form**: `sah <workflow> [args...]` (recommended)
2. **Full form**: `sah flow <workflow> [args...]`

Both forms are equally powerful and use the same underlying dynamic system.

## Command Syntax

### Required Parameters (Positional)

Required workflow parameters are provided as positional arguments in the order they are defined:

```bash
# Plan workflow requires 'plan_filename'
sah plan ideas/feature.md
sah flow plan ideas/feature.md  # Full form

# Custom workflow with multiple required params
sah code-review main feature-x
sah flow code-review main feature-x  # Full form
```

### Optional Parameters (--param)

Optional workflow parameters use `--param key=value`:

```bash
sah custom-workflow --param author=alice --param priority=high
sah flow custom-workflow --param author=alice --param priority=high
```

You can specify `--param` multiple times for multiple optional parameters.

### Common Options

All workflows support these options:

- `--interactive` / `-i`: Enable interactive prompts during execution
- `--dry-run`: Show execution plan without running the workflow
- `--quiet` / `-q`: Suppress progress output

```bash
sah plan ideas/feature.md --interactive --quiet
sah flow plan ideas/feature.md --dry-run
```

## Workflow Discovery

List all available workflows to see what you can execute:

```bash
# Basic list (JSON format)
sah flow list

# Verbose mode (shows parameter details)
sah flow list --verbose

# Different output formats
sah flow list --format yaml
sah flow list --format table
sah flow list --format json
```

### Example Output

```json
{
  "workflows": [
    {
      "name": "implement",
      "description": "Execute the implement workflow for autonomous issue resolution",
      "source": "builtin",
      "parameters": []
    },
    {
      "name": "plan",
      "description": "Execute planning workflow for specific specification files",
      "source": "builtin",
      "parameters": [
        {
          "name": "plan_filename",
          "type": "string",
          "description": "Path to the specification file to process",
          "required": true
        }
      ]
    }
  ]
}
```

## Dynamic Shortcuts

Each workflow automatically gets a top-level command shortcut. This means you can call workflows directly without the `flow` prefix.

### Name Conflict Resolution

If a workflow name conflicts with a reserved command, it gets an underscore prefix:

```bash
# Workflow named "list" becomes "_list"
sah _list [args...]

# Workflow named "flow" becomes "_flow"  
sah _flow [args...]
```

Reserved names include:
- `list` (special case for workflow discovery)
- Top-level commands: `flow`, `agent`, `prompt`, `serve`, `doctor`, `rule`, `validate`

## Common Workflows

### Implement Workflow

Execute the autonomous implementation workflow:

```bash
# Shortcut (recommended)
sah implement --quiet

# Full form
sah flow implement --quiet
```

The implement workflow has no required parameters.

### Plan Workflow

Generate implementation plans from specification files:

```bash
# Shortcut (recommended)
sah plan ideas/feature.md --interactive

# Full form
sah flow plan ideas/feature.md --interactive
```

The plan workflow requires one positional parameter: the path to the specification file.

## Migration from Old Syntax

### Deprecated --var Flag

The `--var` flag is deprecated. Use `--param` instead:

```bash
# Old (deprecated)
sah flow workflow --var key=value

# New (recommended)
sah flow workflow --param key=value
sah workflow --param key=value  # Shortcut
```

### Deprecated Wrapper Commands

The hardcoded `implement` and `plan` wrapper commands are deprecated but still work:

```bash
# Old (deprecated wrappers)
sah implement
sah plan ideas/feature.md

# New (dynamic shortcuts - same syntax, different implementation)
sah implement --quiet
sah plan ideas/feature.md --interactive

# Or use full form
sah flow implement --quiet
sah flow plan ideas/feature.md --interactive
```

Note: The shortcuts work identically to the old commands but use the new dynamic flow system underneath.

## Complete Examples

### Execute Workflow Without Parameters

```bash
# Run the implement workflow in quiet mode
sah implement --quiet
sah flow implement --quiet  # Same thing
```

### Execute Workflow With Required Parameters

```bash
# Run the plan workflow with a spec file
sah plan spec.md
sah flow plan spec.md  # Same thing

# With additional options
sah plan spec.md --interactive --dry-run
```

### Execute Workflow With Optional Parameters

```bash
# Custom workflow with optional parameters
sah my-workflow required-arg --param optional=value --param another=data
sah flow my-workflow required-arg --param optional=value --param another=data
```

### Dry Run Mode

See what a workflow would do without actually executing it:

```bash
sah plan ideas/feature.md --dry-run
sah flow plan ideas/feature.md --dry-run
```

### Interactive Mode

Enable interactive prompts during workflow execution:

```bash
sah plan spec.md --interactive
sah flow plan spec.md --interactive
```

### Quiet Mode

Suppress progress output during execution:

```bash
sah implement --quiet
sah flow implement --quiet
```

### Combining Options

You can combine multiple options:

```bash
sah plan spec.md --interactive --quiet --dry-run
```

## Getting Help

### General Flow Help

```bash
sah flow --help
```

### Workflow-Specific Help

```bash
sah <workflow> --help
sah flow <workflow> --help
```

### List Available Workflows

```bash
sah flow list
sah flow list --verbose  # With parameter details
```

## Troubleshooting

### Workflow Not Found

If you get a "workflow not found" error:

1. Check available workflows: `sah flow list`
2. Verify the workflow name spelling
3. Ensure workflow files are in the correct location

### Missing Required Parameter

If you get a "missing required parameter" error:

1. Use `sah flow list --verbose` to see required parameters
2. Provide required parameters as positional arguments in order
3. Check parameter names match the workflow definition

### Name Conflicts

If a workflow has an underscore prefix (like `_list`), it means the workflow name conflicts with a reserved command. Use the prefixed name:

```bash
sah _list [args...]
```

## Advanced Usage

### Multiple Optional Parameters

```bash
sah workflow required1 required2 \
  --param opt1=value1 \
  --param opt2=value2 \
  --param opt3=value3
```

### JSON Parameter Values

For complex parameter values, you can pass JSON strings:

```bash
sah workflow --param config='{"key":"value","nested":{"data":123}}'
```

### Environment Variables

Workflows can access environment variables. Set them before running:

```bash
export MY_VAR=value
sah workflow
```

## Best Practices

1. **Use shortcuts** - They're more convenient: `sah plan spec.md` vs `sah flow plan spec.md`
2. **Use --dry-run first** - Preview what a workflow will do before executing
3. **Use --verbose for discovery** - `sah flow list --verbose` shows parameter details
4. **Provide clear parameter names** - Use meaningful values for `--param` options
5. **Check workflow completion** - Review output to ensure workflow succeeded

## See Also

- [Flow Migration Guide](FLOW_MIGRATION.md) - Migrating from old syntax
- [Workflow Documentation](https://swissarmyhammer.github.io/swissarmyhammer) - Complete workflow guide
- [MCP Flow Tool](../swissarmyhammer-tools/src/mcp/tools/flow/tool/description.md) - MCP API reference
