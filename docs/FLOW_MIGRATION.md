# Flow System Migration Guide

## What Changed

The flow system has been redesigned for dynamic workflow execution via MCP. This guide helps you migrate from the old patterns to the new system.

## Key Changes at a Glance

### Old System
```bash
sah flow run plan spec.md --var key=value
sah implement
sah plan spec.md
```

### New System
```bash
sah flow plan spec.md --param key=value
sah implement --quiet
sah plan spec.md --interactive
```

**Critical Change**: There is NO "run" subcommand. Use `sah flow <workflow>` directly.

## Breaking Changes

### 1. NO "run" Subcommand

The most important change: the `run` subcommand has been removed.

**Old (BROKEN)**:
```bash
sah flow run plan spec.md
sah flow run implement
```

**New (CORRECT)**:
```bash
sah flow plan spec.md
sah flow implement
```

### 2. Required Parameters Are Positional

Required workflow parameters must be provided as positional arguments, not as `--var` flags.

**Old (BROKEN)**:
```bash
sah flow run plan --var plan_filename=spec.md
```

**New (CORRECT)**:
```bash
sah flow plan spec.md
```

### 3. Optional Parameters Use --param

The `--var` flag is deprecated. Use `--param` for optional parameters.

**Old (DEPRECATED)**:
```bash
sah flow run workflow --var optional=value
```

**New (CORRECT)**:
```bash
sah flow workflow --param optional=value
```

## Non-Breaking Changes

These changes maintain backward compatibility with deprecation warnings:

### 1. Hardcoded Wrapper Commands

The old `implement` and `plan` commands still work but show deprecation warnings.

**Old (works with warning)**:
```bash
sah implement
sah plan spec.md
```

**New (recommended)**:
```bash
sah implement --quiet
sah plan spec.md --interactive
```

Note: The new commands are actually dynamic shortcuts that use the same syntax but route through the new system.

### 2. --var Flag Support

The `--var` flag still works during the transition period but shows warnings.

**Old (works with warning)**:
```bash
sah flow workflow --var key=value
```

**New (recommended)**:
```bash
sah flow workflow --param key=value
```

## Removed Features

These subcommands have been completely removed:

- `sah flow run` - Use `sah flow <workflow>` directly
- `sah flow resume` - No longer supported
- `sah flow status` - No longer supported
- `sah flow logs` - No longer supported
- `sah flow test` - No longer supported

## Migration Steps

### For Command Line Users

#### Step 1: Remove "run" from All Commands

Search your scripts and replace:

```bash
# Find and replace
sed -i 's/sah flow run /sah flow /g' your-script.sh
```

**Before**:
```bash
sah flow run plan spec.md
sah flow run implement
sah flow run custom-workflow
```

**After**:
```bash
sah flow plan spec.md
sah flow implement
sah flow custom-workflow
```

#### Step 2: Use Positional Args for Required Parameters

Identify required parameters with:
```bash
sah flow list --verbose
```

Then convert `--var` to positional arguments:

**Before**:
```bash
sah flow run plan --var plan_filename=spec.md
```

**After**:
```bash
sah flow plan spec.md
```

#### Step 3: Replace --var with --param for Optional Parameters

**Before**:
```bash
sah flow run custom-workflow --var opt1=value1 --var opt2=value2
```

**After**:
```bash
sah flow custom-workflow --param opt1=value1 --param opt2=value2
```

#### Step 4: Use Shortcuts (Optional but Recommended)

**Before**:
```bash
sah flow plan spec.md
sah flow implement --quiet
```

**After (shortcuts)**:
```bash
sah plan spec.md
sah implement --quiet
```

### For Script Automation

#### Update Shell Scripts

```bash
#!/bin/bash

# Old approach (BROKEN)
# sah flow run plan spec.md --var key=value

# New approach (WORKS)
sah flow plan spec.md --param key=value

# Or use shortcuts
sah plan spec.md --param key=value
```

#### Suppress Deprecation Warnings

If you need time to migrate but don't want warnings:

```bash
# Add this flag during transition (if implemented)
sah flow plan spec.md --no-deprecation-warning
```

Note: This is a temporary solution. Plan to migrate fully.

### For MCP Clients

#### Old MCP Pattern (If It Existed)

If you were using a hypothetical old MCP tool:

```json
{
  "name": "workflow_execute",
  "arguments": {
    "workflow": "plan",
    "variables": {
      "plan_filename": "spec.md"
    }
  }
}
```

#### New MCP Pattern

Use the unified `flow` tool:

```json
{
  "name": "flow",
  "arguments": {
    "flow_name": "plan",
    "parameters": {
      "plan_filename": "spec.md"
    }
  }
}
```

#### Discover Workflows

```json
{
  "name": "flow",
  "arguments": {
    "flow_name": "list",
    "verbose": true
  }
}
```

#### Handle Notifications

Subscribe to MCP notifications to track workflow progress:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "<workflow_run_id>",
    "value": 50,
    "message": "Completed state: parse_spec"
  }
}
```

See the [Flow Tool Description](https://swissarmyhammer.github.io/swissarmyhammer/mcp/flow) for complete notification details.

## Migration Examples

### Example 1: Simple Workflow

**Before**:
```bash
sah flow run implement
```

**After**:
```bash
sah flow implement
# Or use shortcut
sah implement
```

### Example 2: Workflow with Required Parameter

**Before**:
```bash
sah flow run plan --var plan_filename=ideas/feature.md
```

**After**:
```bash
sah flow plan ideas/feature.md
# Or use shortcut
sah plan ideas/feature.md
```

### Example 3: Workflow with Optional Parameters

**Before**:
```bash
sah flow run custom-workflow --var author=alice --var priority=high
```

**After**:
```bash
sah flow custom-workflow --param author=alice --param priority=high
# Or use shortcut
sah custom-workflow --param author=alice --param priority=high
```

### Example 4: Multiple Required Parameters

**Before**:
```bash
sah flow run code-review --var target=main --var source=feature-x
```

**After**:
```bash
sah flow code-review main feature-x
# Or use shortcut
sah code-review main feature-x
```

### Example 5: Mixed Required and Optional

**Before**:
```bash
sah flow run deploy --var environment=prod --var version=1.2.3 --var confirm=true
```

**After** (assuming environment and version are required, confirm is optional):
```bash
sah flow deploy prod 1.2.3 --param confirm=true
# Or use shortcut
sah deploy prod 1.2.3 --param confirm=true
```

## Deprecation Timeline

### Current Release
- New dynamic flow system available
- Old syntax shows deprecation warnings
- `--var` flag deprecated but functional
- Hardcoded wrappers deprecated but functional

### Next Release (Planned)
- Deprecation warnings become more prominent
- Documentation updated to remove old patterns
- Migration guide prominently featured

### Future Release (TBD)
- Deprecated patterns removed entirely
- Only new syntax supported
- Clean codebase with dynamic system only

## Common Migration Issues

### Issue: "Unknown subcommand 'run'"

**Cause**: Using the old `sah flow run` syntax.

**Solution**: Remove `run` from the command:
```bash
# Wrong
sah flow run plan spec.md

# Right
sah flow plan spec.md
```

### Issue: "Missing required parameter"

**Cause**: Not providing required parameters as positional arguments.

**Solution**: Check workflow parameters and provide them positionally:
```bash
# Check parameters
sah flow list --verbose

# Provide required params positionally
sah flow plan spec.md  # Not --var plan_filename=spec.md
```

### Issue: "Workflow not found"

**Cause**: Typo in workflow name or workflow doesn't exist.

**Solution**: List available workflows:
```bash
sah flow list
```

### Issue: "Unknown flag --var"

**Cause**: Using deprecated `--var` flag in a future version where it's removed.

**Solution**: Use `--param` instead:
```bash
# Old
sah flow workflow --var key=value

# New
sah flow workflow --param key=value
```

## Getting Help

### Check Workflow Parameters

```bash
sah flow list --verbose
```

### Test with Dry Run

```bash
sah flow workflow args... --dry-run
```

### Read the Documentation

- [CLI Flow Usage Guide](./CLI_FLOW_USAGE.md)
- [Flow Tool Description](https://swissarmyhammer.github.io/swissarmyhammer/mcp/flow)
- [SwissArmyHammer Documentation](https://swissarmyhammer.github.io/swissarmyhammer)

### Report Issues

If you encounter migration problems:

1. Check this guide for solutions
2. Verify your command syntax with `sah flow list --verbose`
3. Report issues at: https://github.com/swissarmyhammer/swissarmyhammer/issues

## Quick Reference

### Command Transformation Table

| Old Command | New Command | Notes |
|-------------|-------------|-------|
| `sah flow run <workflow>` | `sah flow <workflow>` | Remove "run" |
| `sah flow run <workflow> --var k=v` | `sah flow <workflow> --param k=v` | Use --param |
| `sah flow run plan --var plan_filename=f` | `sah flow plan f` | Positional args |
| `sah implement` | `sah implement` | Works but use options |
| `sah plan f` | `sah plan f` | Works but use options |

### Parameter Transformation Table

| Old Style | New Style | Type |
|-----------|-----------|------|
| `--var required=value` | `value` (positional) | Required param |
| `--var optional=value` | `--param optional=value` | Optional param |
| N/A | `--interactive` | Common option |
| N/A | `--dry-run` | Common option |
| N/A | `--quiet` | Common option |

## Summary

The new flow system is more powerful and flexible than the old approach:

- **No "run" subcommand** - Direct workflow execution
- **Positional required parameters** - More intuitive CLI
- **Dynamic shortcuts** - Convenient top-level commands
- **MCP notifications** - Real-time progress tracking
- **Unified discovery** - `flow_name="list"` for exploration

Migration is straightforward: remove "run", use positional args for required parameters, and replace `--var` with `--param` for optional parameters.
