# Changelog

All notable changes to SwissArmyHammer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Dynamic Flow MCP Tool**: Unified `flow` tool for workflow discovery and execution via MCP protocol
  - Special case: `flow_name="list"` discovers all available workflows
  - Execute workflows by setting `flow_name` to the workflow name
  - Dynamic schema generation with workflow names in enum
- **MCP Notification Support**: Real-time progress tracking for workflow execution
  - Flow start notification when workflow begins
  - State start notification when entering each workflow state
  - State complete notification when exiting each workflow state  
  - Flow complete notification on successful completion
  - Flow error notification on execution failure
  - Progress percentages calculated from state position
  - Unique workflow run ID as notification token
- **Dynamic CLI Shortcuts**: Automatic top-level commands for all workflows
  - Each workflow gets a shortcut: `sah <workflow>` in addition to `sah flow <workflow>`
  - Name conflict resolution with underscore prefix for reserved names
  - Consistent parameter handling across all workflows
- **Positional Parameter Support**: Required workflow parameters as positional arguments
  - More intuitive CLI: `sah plan spec.md` instead of `--var plan_filename=spec.md`
  - Order matches workflow parameter definition order
- **--param Flag**: New flag for optional workflow parameters
  - Replaces deprecated `--var` flag
  - Standard convention: `--param key=value`
  - Can be specified multiple times for multiple parameters
- **Comprehensive Documentation**:
  - CLI Flow Usage Guide (`docs/CLI_FLOW_USAGE.md`)
  - Flow Migration Guide (`docs/FLOW_MIGRATION.md`)
  - Enhanced MCP tool description with notification details
  - README section on workflow execution

### Changed
- **Flow Command Structure**: Direct workflow execution without "run" subcommand
  - Use `sah flow <workflow>` instead of `sah flow run <workflow>`
  - Breaking change: old syntax no longer supported
- **Parameter Convention**: Required params are positional, optional use `--param`
  - Required parameters: `sah workflow required1 required2`
  - Optional parameters: `sah workflow --param opt1=value --param opt2=value`
  - Breaking change for workflows with required parameters
- **Workflow Discovery**: Use `sah flow list` instead of separate discovery commands
  - `--verbose` flag shows detailed parameter information
  - Multiple output formats: json, yaml, table

### Removed
- **flow run Subcommand**: Removed in favor of direct execution
  - Use `sah flow <workflow>` instead of `sah flow run <workflow>`
- **flow resume Subcommand**: No longer supported
- **flow status Subcommand**: No longer supported  
- **flow logs Subcommand**: No longer supported
- **flow test Subcommand**: No longer supported

### Deprecated
- **Hardcoded Wrapper Commands**: `implement` and `plan` commands are deprecated
  - Still functional but show deprecation warnings
  - Use dynamic shortcuts instead (same syntax, different implementation)
  - Will be removed in a future release
- **--var Flag**: Deprecated in favor of `--param`
  - Still functional but shows deprecation warnings
  - Use `--param key=value` instead of `--var key=value`
  - Will be removed in a future release

### Migration

See [Flow Migration Guide](docs/FLOW_MIGRATION.md) for complete migration instructions.

**Key Migration Steps**:

1. **Remove "run" subcommand**:
   - OLD: `sah flow run plan spec.md`
   - NEW: `sah flow plan spec.md`

2. **Use positional args for required parameters**:
   - OLD: `sah flow run plan --var plan_filename=spec.md`
   - NEW: `sah flow plan spec.md`

3. **Replace --var with --param for optional parameters**:
   - OLD: `--var key=value`
   - NEW: `--param key=value`

4. **Use shortcuts** (optional but recommended):
   - `sah plan spec.md` instead of `sah flow plan spec.md`

### Technical Details

#### MCP Tool Schema

The `flow` tool dynamically generates its schema based on available workflows:

```json
{
  "flow_name": {
    "type": "string",
    "enum": ["list", "implement", "plan", ...]
  }
}
```

The `list` value is always included as a special case for workflow discovery.

#### Notification Protocol

Notifications follow the MCP progress notification protocol:

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

Metadata includes flow name, state information, and execution context.

#### Shortcut Generation

Shortcuts are generated at runtime by scanning available workflows:
- Reserved names get underscore prefix: `_list`, `_flow`
- All other workflows get direct shortcuts: `plan`, `implement`
- Both shortcut and full form use the same underlying implementation

## [Previous Versions]

Previous changelog entries will be added as releases are tagged.
