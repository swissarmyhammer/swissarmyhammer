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
- `metadata`: Structured data about the event (flow_name, state info, parameters)

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

The flow system provides two ways to execute workflows:

1. **Full form**: `sah flow <workflow> [args...]`
2. **Shortcut form**: `sah <workflow> [args...]` (recommended)

## Command Syntax

### Required Parameters (Positional)

Required workflow parameters are positional arguments:

```bash
# Plan workflow requires 'plan_filename'
sah flow plan ideas/feature.md
sah plan ideas/feature.md  # Shortcut

# Custom workflow with multiple required params
sah flow code-review main feature-x
sah code-review main feature-x  # Shortcut
```

### Optional Parameters (--param)

Optional workflow parameters use `--param key=value`:

```bash
sah flow custom-workflow --param author=alice --param priority=high
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

# Workflow named "flow" becomes "_flow"
sah _flow [args...]
```

Reserved names: `list`, plus all top-level commands (`flow`, `agent`, `prompt`, `serve`, `doctor`, `rule`, `validate`)

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

The hardcoded `implement` and `plan` wrapper commands are deprecated:

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

Note: Shortcuts work the same as old commands, but use the new dynamic system.

## Examples

### Execute Implement Workflow

```bash
# Full form
sah flow implement --quiet

# Shortcut (recommended)
sah implement --quiet
```

### Execute Plan Workflow

```bash
# Full form
sah flow plan ideas/feature.md --interactive

# Shortcut (recommended)
sah plan ideas/feature.md --interactive
```

### Execute Custom Workflow

```bash
# With required and optional parameters
sah flow my-workflow required-arg --param optional=value

# Shortcut
sah my-workflow required-arg --param optional=value
```

### Dry Run

```bash
# See what workflow would do without executing
sah plan ideas/feature.md --dry-run
```
```

### 3. Update Main README

Update repository README with flow system overview (section to add):

```markdown
## Workflow Execution

SwissArmyHammer provides dynamic workflow execution through the flow system.

### Quick Start

```bash
# List available workflows
sah flow list

# Execute a workflow (full form)
sah flow plan ideas/feature.md

# Execute a workflow (shortcut)
sah plan ideas/feature.md
```

### Documentation

- [Flow Usage Guide](docs/CLI_FLOW_USAGE.md)
- [Migration Guide](docs/FLOW_MIGRATION.md)
- [MCP Tool Documentation](swissarmyhammer-tools/src/mcp/tools/flow/description.md)
```

### 4. Create Migration Guide

Create `docs/FLOW_MIGRATION.md`:

```markdown
# Flow System Migration Guide

## What Changed

The flow system has been redesigned for dynamic workflow execution via MCP.

### Key Changes

1. **No "run" Subcommand**: `sah flow [workflow]` not `sah flow run [workflow]`
2. **Parameter Convention**: Required params are positional, optional use `--param`
3. **Dynamic Shortcuts**: Workflows automatically get top-level commands
4. **MCP Notifications**: Long-running workflows send progress updates
5. **Removed Commands**: Deprecated subcommands removed

### Breaking Changes

- NO "run" subcommand: Use `sah flow [workflow]` directly
- `--var` flag deprecated, use `--param` instead
- Required parameters must be positional

### Non-Breaking Changes

- Hardcoded `implement` and `plan` commands still work (with warnings)
- `--var` still works during transition (with warnings)
- Shortcuts have same names as old commands

## Migration Steps

### For End Users

1. **Remove "run" from commands**:
   - OLD: `sah flow run plan spec.md`
   - NEW: `sah flow plan spec.md`

2. **Use positional args for required parameters**:
   - Already works: `sah plan spec.md`

3. **Replace --var with --param for optional parameters**:
   - OLD: `--var key=value`
   - NEW: `--param key=value`

4. **Use shortcuts** (optional but recommended):
   - `sah plan file.md` instead of `sah flow plan file.md`

### For Scripts

- Add `--no-deprecation-warning` to suppress warnings if needed
- Plan to migrate away from deprecated syntax

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

- **Now**: New system available, old syntax deprecated
- **Next release**: Deprecation warnings added
- **Future release**: Deprecated patterns removed

## Getting Help

- Use `sah flow --help` for command help
- Use `sah flow list --verbose` to see workflow parameters
- Check workflow-specific help: `sah <workflow> --help`
```

### 5. Update Changelog

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
- Flow command now takes workflow name directly: `sah flow [workflow]` not `sah flow run [workflow]`
- Flow parameter convention: required params are positional
- `--var` flag deprecated in favor of `--param`

### Removed
- `flow run` subcommand (use `flow [workflow]` directly)
- `flow resume` subcommand
- `flow status` subcommand
- `flow logs` subcommand
- `flow test` subcommand

### Deprecated
- Hardcoded `implement` and `plan` wrapper commands (use shortcuts instead)
- `--var` flag (use `--param` instead)

### Migration
See [Flow Migration Guide](docs/FLOW_MIGRATION.md) for details.
```

### 6. Add Code Comments

Add comprehensive code documentation:

- Document `FlowTool` struct and methods
- Document notification system
- Document shortcut generation
- Document parameter mapping
- Document special case handling for "list"

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
- [ ] Migration guide clearly explains NO "run" subcommand
- [ ] README updated with flow overview
- [ ] Changelog documents all changes
- [ ] Code comments explain implementation
- [ ] All examples use correct syntax (no "flow run")
- [ ] Documentation is consistent across all files

## Estimated Changes

~600 lines of documentation

## Proposed Solution

After reviewing the current implementation, I can see that:

1. **The FlowTool is fully implemented** with notification support and comprehensive tests
2. **The tool description exists** but needs enhancement with notification details
3. **No docs/ directory exists** - needs to be created
4. **README has recent changes** - needs workflow section added carefully
5. **No CHANGELOG.md exists** - needs to be created

### Implementation Approach

I will create comprehensive documentation following this strategy:

#### 1. Enhanced Flow Tool Description
- Add detailed section on MCP notifications (flow start, state transitions, completion, errors)
- Document notification structure with token, progress, metadata
- Clarify that "list" is a special value for flow_name
- Add examples showing notification sequences
- Keep existing structure but expand notification coverage

#### 2. CLI Usage Guide (docs/CLI_FLOW_USAGE.md)
- Document the shortcut form: `sah <workflow>` (primary recommendation)
- Document the full form: `sah flow <workflow>` (alternative)
- Explain positional parameters for required workflow parameters
- Explain `--param key=value` for optional parameters
- Show workflow discovery with `sah flow list`
- Include migration notes from deprecated `--var` syntax
- Provide comprehensive examples for common scenarios

#### 3. Migration Guide (docs/FLOW_MIGRATION.md)
- **Key Point**: NO "run" subcommand (this is critical - it's `sah flow <workflow>` not `sah flow run <workflow>`)
- Document parameter convention changes (positional required, `--param` for optional)
- Show before/after examples for all migration scenarios
- Explain deprecation timeline and warnings
- Cover both CLI and MCP client migration

#### 4. README Update
- Add "Workflow Execution" section after the "Get Started" section
- Include quick start examples with both shortcut and full form
- Link to detailed documentation guides
- Keep it concise - detailed info goes in dedicated docs

#### 5. CHANGELOG.md
- Create new file with comprehensive entry
- Document all additions (dynamic flow tool, notifications, shortcuts, positional params)
- Document changes (NO "run" subcommand, parameter conventions)
- Document removals (flow subcommands: run, resume, status, logs, test)
- Document deprecations (hardcoded wrappers, `--var` flag)
- Reference migration guide

#### 6. Code Documentation
- The existing code is already well-documented with comprehensive tests
- I'll review and add any missing details about notification behavior
- Focus on ensuring notification timing and progress calculation are clear

### Key Documentation Principles

1. **Emphasize NO "run" subcommand** - this is critical and must be clear everywhere
2. **Show both forms** - shortcut (`sah plan file.md`) and full (`sah flow plan file.md`)
3. **Recommend shortcuts** - they're more convenient and equally powerful
4. **Be consistent** - use the same terminology and examples across all docs
5. **Provide examples** - every concept needs a working example
6. **Explain notifications** - MCP clients need to understand the notification flow

### Files to Create/Modify

**Create:**
- `docs/CLI_FLOW_USAGE.md` (~200 lines)
- `docs/FLOW_MIGRATION.md` (~150 lines)
- `CHANGELOG.md` (~80 lines)

**Modify:**
- `swissarmyhammer-tools/src/mcp/tools/flow/tool/description.md` (expand notifications section ~50 lines added)
- `README.md` (add workflow section ~30 lines)
- Review code comments in `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs` (minimal changes)

**Total estimated**: ~500-600 lines of documentation

This approach ensures complete, consistent documentation that helps users understand and migrate to the new dynamic flow system.
## Implementation Notes

### Completed Tasks

#### 1. Enhanced Flow Tool Description ✅
- Updated `swissarmyhammer-tools/src/mcp/tools/flow/tool/description.md`
- Added comprehensive "Progress Notifications" section
- Documented all notification types: Flow Start, State Start, State Complete, Flow Complete, Flow Error
- Included notification structure examples with JSON
- Explained progress calculation methodology
- Added example notification sequences for both success and failure cases
- Total addition: ~150 lines of detailed notification documentation

#### 2. CLI Usage Guide ✅
- Created `docs/CLI_FLOW_USAGE.md` (~250 lines)
- Documented both shortcut and full command forms
- Explained positional parameters for required workflow params
- Explained `--param key=value` for optional parameters
- Included workflow discovery with `sah flow list`
- Provided migration notes from deprecated `--var` syntax
- Added comprehensive examples for all common scenarios
- Included troubleshooting and best practices sections

#### 3. Migration Guide ✅
- Created `docs/FLOW_MIGRATION.md` (~400 lines)
- **Emphasized**: NO "run" subcommand (critical breaking change)
- Documented all breaking changes with before/after examples
- Covered non-breaking changes with deprecation warnings
- Listed removed features (run, resume, status, logs, test subcommands)
- Provided step-by-step migration instructions for CLI users, scripts, and MCP clients
- Included complete examples for common migration scenarios
- Added deprecation timeline and common issue troubleshooting
- Created quick reference tables for command and parameter transformations

#### 4. README Update ✅
- Updated crypto workflow example to use new syntax (removed `flow run`)
- Added "Workflow Execution" section after existing content
- Included quick start examples showing both forms
- Documented key features (discovery, shortcuts, progress tracking, parameters)
- Highlighted built-in workflows (implement, plan)
- Linked to all detailed documentation guides
- Total addition: ~50 lines with clear, concise overview

#### 5. CHANGELOG.md ✅
- Created comprehensive `CHANGELOG.md` (~150 lines)
- Documented additions: dynamic flow tool, MCP notifications, shortcuts, positional params, documentation
- Documented changes: NO "run" subcommand, parameter conventions, workflow discovery
- Documented removals: flow subcommands (run, resume, status, logs, test)
- Documented deprecations: hardcoded wrappers, `--var` flag
- Included migration guide reference with key migration steps
- Added technical details section explaining MCP tool schema, notification protocol, and shortcut generation

#### 6. Code Documentation Review ✅
- Reviewed `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs`
- Found existing documentation is comprehensive and well-structured:
  - Module-level documentation
  - Function-level documentation with detailed arguments and returns
  - Inline comments explaining notification behavior
  - Comments noting progress calculation limitations for loops/conditionals
  - Helper function documentation
  - Comprehensive test suite with 20+ tests covering all scenarios
- No changes needed - code documentation already meets high standards

### Documentation Statistics

**Files Created:**
- `docs/CLI_FLOW_USAGE.md` (250 lines)
- `docs/FLOW_MIGRATION.md` (400 lines)
- `CHANGELOG.md` (150 lines)

**Files Modified:**
- `swissarmyhammer-tools/src/mcp/tools/flow/tool/description.md` (+150 lines)
- `README.md` (+50 lines with workflow section, updated examples)

**Total Documentation:** ~1000 lines of comprehensive, consistent documentation

### Key Documentation Themes

1. **NO "run" Subcommand** - Emphasized throughout all documents as the critical breaking change
2. **Both Forms Shown** - Shortcut (`sah plan file.md`) and full (`sah flow plan file.md`) consistently demonstrated
3. **Shortcuts Recommended** - Highlighted as the preferred approach for convenience
4. **Consistent Examples** - Same patterns and terminology across all documentation
5. **MCP Notifications** - Thoroughly explained for client integration

### Quality Assurance

- All documentation uses consistent terminology
- Examples are realistic and match actual implementation
- Migration path is clear with before/after comparisons
- Troubleshooting sections address common issues
- Links between documents create a cohesive documentation set
- Code examples use correct syntax (no deprecated patterns)
- All acceptance criteria from the original issue met