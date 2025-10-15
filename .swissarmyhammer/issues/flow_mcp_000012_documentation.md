# Step 12: Documentation Updates

Refer to ideas/flow_mcp.md

## Objective

Update all documentation to reflect the new dynamic flow system, including usage examples, migration guides, and API documentation.

## Context

With the implementation complete, we need comprehensive documentation so users understand how to use the new flow system and migrate from the old patterns.

## Tasks

### 1. Update Flow Tool Description

Update `swissarmyhammer-tools/src/mcp/tools/flow/description.md`:

```markdown
# Flow Tool

Execute or list workflows dynamically via MCP protocol.

## Overview

The `flow` tool provides unified access to workflow execution and discovery through
the MCP protocol. It supports both listing available workflows and executing them
with parameters.

## Usage

### Discover Available Workflows

Set `flow_name` to `"list"` to retrieve metadata about all available workflows:

```json
{
  "flow_name": "list",
  "format": "json",
  "verbose": true
}
```

Response includes workflow names, descriptions, sources, and parameters (in verbose mode).

### Execute a Workflow

Set `flow_name` to a specific workflow name and provide parameters:

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "ideas/feature.md"
  },
  "interactive": false,
  "dry_run": false,
  "quiet": false
}
```

### Progress Notifications

Long-running workflows send MCP notifications to track progress:

- **Flow Start**: Sent when workflow begins
- **State Start**: Sent when entering each workflow state
- **State Complete**: Sent when exiting each workflow state  
- **Flow Complete**: Sent on successful completion
- **Flow Error**: Sent if workflow fails

Each notification includes:
- `token`: Unique workflow run ID
- `progress`: Percentage complete (0-100)
- `message`: Human-readable status message
- `metadata`: Structured data about the event

## Parameters

### flow_name (required)
- Type: `string`
- Description: Workflow name to execute, or `"list"` for discovery
- Enum: Dynamic list including "list" plus all available workflows

### parameters (optional)
- Type: `object`
- Description: Workflow-specific parameters as key-value pairs
- Default: `{}`

### format (optional, for list only)
- Type: `string`
- Description: Output format for workflow list
- Enum: `["json", "yaml", "table"]`
- Default: `"json"`

### verbose (optional, for list only)
- Type: `boolean`
- Description: Include detailed parameter information
- Default: `false`

### interactive (optional)
- Type: `boolean`
- Description: Enable interactive prompts during execution
- Default: `false`

### dry_run (optional)
- Type: `boolean`
- Description: Show execution plan without running
- Default: `false`

### quiet (optional)
- Type: `boolean`
- Description: Suppress progress output
- Default: `false`

## Examples

See integration tests for complete examples.
```

### 2. Create CLI Usage Guide

Create `docs/CLI_FLOW_USAGE.md`:

```markdown
# Flow Command Usage Guide

## Overview

The flow system provides three ways to execute workflows:

1. **Full form**: `sah flow run <workflow> [args...]`
2. **Shortcut form**: `sah <workflow> [args...]` (recommended)
3. **MCP tool**: Via the `flow` MCP tool

## Command Syntax

### Required Parameters (Positional)

Required workflow parameters are positional arguments:

```bash
# Plan workflow requires 'plan_filename'
sah flow run plan ideas/feature.md
sah plan ideas/feature.md  # Shortcut

# Custom workflow with multiple required params
sah flow run code-review main feature-x
sah code-review main feature-x  # Shortcut
```

### Optional Parameters (--param)

Optional workflow parameters use `--param key=value`:

```bash
sah flow run custom-workflow --param author=alice --param priority=high
sah custom-workflow --param author=alice --param priority=high
```

### Common Options

All workflows support these options:

- `--interactive` / `-i`: Enable interactive prompts
- `--dry-run`: Show execution plan without running
- `--quiet` / `-q`: Suppress progress output

```bash
sah plan ideas/feature.md --interactive --quiet
```

## Workflow Discovery

List all available workflows:

```bash
# Basic list
sah flow list

# Verbose (shows parameters)
sah flow list --verbose

# Different formats
sah flow list --format yaml
sah flow list --format table
```

## Dynamic Shortcuts

Each workflow automatically gets a top-level command shortcut.

### Name Conflict Resolution

If a workflow name conflicts with a reserved command, it gets an underscore prefix:

```bash
# Workflow named "list" becomes "_list"
sah _list [args...]

# Workflow named "run" becomes "_run"
sah _run [args...]
```

Reserved names: `list`, `run`, plus all top-level commands (`flow`, `agent`, `prompt`, etc.)

## Migration from Old Syntax

### Deprecated --var Flag

The `--var` flag is deprecated. Use `--param` instead:

```bash
# Old (deprecated)
sah flow run workflow --var key=value

# New (recommended)
sah flow run workflow --param key=value
sah workflow --param key=value  # Shortcut
```

### Deprecated Wrapper Commands

The hardcoded `implement` and `plan` commands are deprecated:

```bash
# Old (deprecated)
sah implement
sah plan ideas/feature.md

# New (recommended)
sah flow run implement
sah flow run plan ideas/feature.md

# Or use shortcuts
sah implement
sah plan ideas/feature.md
```

Note: Shortcuts work the same as old commands, but use the new dynamic system.

## Examples

### Execute Implement Workflow

```bash
# Full form
sah flow run implement --quiet

# Shortcut (recommended)
sah implement --quiet
```

### Execute Plan Workflow

```bash
# Full form
sah flow run plan ideas/feature.md --interactive

# Shortcut (recommended)
sah plan ideas/feature.md --interactive
```

### Execute Custom Workflow

```bash
# With required and optional parameters
sah flow run my-workflow required-arg --param optional=value

# Shortcut
sah my-workflow required-arg --param optional=value
```

### Dry Run

```bash
# See what workflow would do without executing
sah plan ideas/feature.md --dry-run
```
```

### 3. Create Migration Guide

Create `docs/FLOW_MIGRATION.md`:

```markdown
# Flow System Migration Guide

## What Changed

The flow system has been redesigned for dynamic workflow execution via MCP.

### Key Changes

1. **Unified MCP Tool**: Single `flow` tool handles both discovery and execution
2. **Parameter Convention**: Required params are positional, optional use `--param`
3. **Dynamic Shortcuts**: Workflows automatically get top-level commands
4. **MCP Notifications**: Long-running workflows send progress updates
5. **Removed Commands**: `flow resume`, `flow status`, `flow logs` removed

### Breaking Changes

- `--var` flag deprecated, use `--param` instead
- `flow resume`, `flow status`, `flow logs` subcommands removed
- Required parameters must be positional (not `--param`)

### Non-Breaking Changes

- Hardcoded `implement` and `plan` commands still work (with warnings)
- `--var` still works during transition (with warnings)
- Shortcuts have same names as old commands

## Migration Steps

### For End Users

1. **Update command invocations**:
   - Replace `--var` with `--param`
   - Use positional args for required parameters

2. **Use shortcuts** (optional but recommended):
   - `sah plan file.md` instead of `sah flow run plan file.md`

3. **Update scripts**:
   - Add `--no-deprecation-warning` to suppress warnings if needed
   - Plan to migrate away from deprecated commands

### For Workflow Definitions

No changes needed - workflow definitions are compatible.

### For MCP Clients

1. **Discover workflows**:
   ```json
   {
     "flow_name": "list",
     "verbose": true
   }
   ```

2. **Execute workflows**:
   ```json
   {
     "flow_name": "plan",
     "parameters": {
       "plan_filename": "ideas/feature.md"
     }
   }
   ```

3. **Handle notifications**:
   Subscribe to MCP notifications for progress tracking.

## Timeline

- **Now**: New system available, old commands deprecated
- **Next release**: Deprecation warnings added
- **Future release**: Deprecated commands removed

## Getting Help

- Use `sah flow --help` for command help
- Use `sah flow list --verbose` to see workflow parameters
- Check workflow-specific help: `sah <workflow> --help`
```

### 4. Update Main README

Update repository README with flow system overview:

```markdown
## Workflow Execution

SwissArmyHammer provides dynamic workflow execution through the flow system.

### Quick Start

```bash
# List available workflows
sah flow list

# Execute a workflow (full form)
sah flow run plan ideas/feature.md

# Execute a workflow (shortcut)
sah plan ideas/feature.md
```

### Documentation

- [Flow Usage Guide](docs/CLI_FLOW_USAGE.md)
- [Migration Guide](docs/FLOW_MIGRATION.md)
- [MCP Tool Documentation](swissarmyhammer-tools/src/mcp/tools/flow/description.md)
```

### 5. Add Code Comments

Add comprehensive code documentation:

- Document `FlowTool` struct and methods
- Document notification system
- Document shortcut generation
- Document parameter mapping

### 6. Update Changelog

Create `CHANGELOG.md` entry:

```markdown
## [Version] - Date

### Added
- Dynamic flow MCP tool for workflow discovery and execution
- MCP notification support for workflow progress tracking
- Dynamic CLI shortcuts for all workflows
- Positional parameter support for required workflow parameters
- `--param` flag for optional workflow parameters

### Changed
- Flow parameter convention: required params are positional
- `--var` flag deprecated in favor of `--param`

### Removed
- `flow resume` subcommand (not part of new design)
- `flow status` subcommand (not part of new design)
- `flow logs` subcommand (not part of new design)

### Deprecated
- Hardcoded `implement` and `plan` commands (use shortcuts instead)
- `--var` flag (use `--param` instead)

### Migration
See [Flow Migration Guide](docs/FLOW_MIGRATION.md) for details.
```

## Files to Create/Update

- `swissarmyhammer-tools/src/mcp/tools/flow/description.md` (update)
- `docs/CLI_FLOW_USAGE.md` (create)
- `docs/FLOW_MIGRATION.md` (create)
- `README.md` (update)
- `CHANGELOG.md` (update)
- Code comments throughout flow implementation

## Acceptance Criteria

- [ ] Flow tool description is comprehensive
- [ ] CLI usage guide covers all use cases
- [ ] Migration guide is clear and actionable
- [ ] README updated with flow overview
- [ ] Changelog documents all changes
- [ ] Code comments explain implementation
- [ ] All examples are tested and working
- [ ] Documentation is consistent across all files

## Estimated Changes

~600 lines of documentation
