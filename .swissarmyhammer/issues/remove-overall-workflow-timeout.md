# Remove Overall Workflow Timeout Configuration

## Problem

The codebase currently supports an overall workflow timeout via `timeout_ms` in workflow YAML configurations. This creates redundant timeout mechanisms when individual action timeouts are sufficient to prevent workflows from hanging.

## Current Usage

The `timeout_ms` configuration appears in:
- `doc/src/04-workflows/workflows.md` - Multiple examples
- `doc/src/examples/advanced.md` - Advanced workflow examples  
- Various workflow configuration examples throughout documentation
- Examples range from 30 seconds to 1 week

## Rationale for Removal

### Action Timeouts Are Sufficient
- With unified `action_timeout` (1 hour default), individual actions cannot hang indefinitely
- Each action has its own timeout protection
- Workflows naturally complete when all actions complete or timeout

### Reduces Configuration Complexity
- Eliminates one more timeout parameter users must understand
- Removes potential confusion between action-level and workflow-level timeouts
- Simplifies workflow YAML structure

### Avoids Timeout Conflicts
- No more scenarios where workflow timeout conflicts with action timeout
- No need to calculate appropriate workflow timeout based on number of actions
- Single point of timeout control at the action level

## Implementation Tasks

### 1. Remove from Workflow Parsing
- Find and remove `timeout_ms` parsing from workflow configuration
- Remove any workflow execution timeout enforcement code
- Update workflow execution engine to rely solely on action timeouts

### 2. Update Documentation
- Remove all `timeout_ms` examples from documentation
- Update workflow configuration reference
- Update any tutorials or guides mentioning workflow timeouts

### 3. Clean Up Configuration Examples
- Remove `timeout_ms` from all example YAML files
- Update any templates that include workflow timeouts
- Ensure examples in `doc/` and `examples/` directories are updated

### 4. Update Tests
- Remove any tests that verify workflow-level timeout behavior
- Update integration tests that may reference `timeout_ms`
- Ensure workflow execution tests work without overall timeout

## Benefits After Removal

- Simpler workflow configuration
- Clearer timeout semantics (action-level only)
- Reduced cognitive overhead for users
- Elimination of timeout hierarchy complexity
- More predictable workflow behavior

## Files to Search and Update

- Workflow parsing/execution code
- All documentation in `doc/src/04-workflows/`
- All examples in `doc/src/examples/`
- Any configuration templates
- Workflow YAML schema definitions
- Integration and unit tests for workflow execution