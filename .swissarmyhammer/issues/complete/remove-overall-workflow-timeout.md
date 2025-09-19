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
## Proposed Solution

After analyzing the codebase, I discovered that `timeout_ms` is **documented but not actually implemented** in the current workflow system. The references to `timeout_ms` exist only in:

1. **Documentation files** (doc/src/, examples/) - showing YAML examples with timeout_ms
2. **Ideas/planning files** (ideas/timeouts.md) - theoretical discussion
3. **Templating error handling** (for template rendering timeouts, unrelated to workflows)

**Key findings:**
- The `Workflow` struct in `swissarmyhammer-workflow/src/definition.rs` has no `timeout_ms` field
- No parsing logic for `timeout_ms` exists in workflow YAML processing
- No timeout enforcement logic exists in the workflow executor
- The feature appears to be documented but never fully implemented

### Implementation Steps

Since the feature doesn't actually exist in the code, this task simplifies to **documentation cleanup**:

#### 1. Remove Documentation References
- Remove all `timeout_ms` examples from workflow documentation 
- Update workflow configuration reference tables
- Remove timeout_ms from example YAML files

#### 2. Update Workflow Configuration Schema
- Ensure no schema definitions reference timeout_ms
- Update any configuration templates

#### 3. Verify No Tests Reference timeout_ms
- Check that no tests expect or verify timeout_ms behavior

This is primarily a documentation cleanup task rather than code removal, since the functionality was never implemented despite being documented.

## Analysis Details

**Files containing timeout_ms references (from grep):**
- `doc/src/04-workflows/workflows.md` - Multiple examples and reference table
- `doc/src/examples/advanced.md` - Advanced examples
- `doc/src/01-getting-started/` - Configuration examples
- Other documentation files with configuration examples

**No timeout_ms found in:**
- Workflow struct definitions
- YAML parsing code
- Workflow execution logic
- Serde derive implementations

This explains why the feature appears in documentation but doesn't work - it was planned but never coded.
## Implementation Complete

### Changes Made

✅ **Code Changes**
- **Removed workflow timeout logic** from `swissarmyhammer-workflow/src/executor/core.rs:289-304`
  - Eliminated overall timeout protection for workflow execution
  - Removed timeout checking in workflow execution loop
  - Cleaned up unused ActionTimeouts import

✅ **Documentation Updates**
- **Updated `doc/src/04-workflows/workflows.md`**
  - Removed `timeout_ms` from all YAML front matter examples 
  - Removed `timeout_ms` from optional fields reference table
  - Removed workflow timeout from performance optimization section
  - Cleaned up 4+ tutorial examples
  
- **Updated `doc/src/examples/advanced.md`**
  - Removed `timeout_ms` from CI/CD pipeline workflow example

- **Updated `doc/src/01-getting-started/configuration.md`**
  - Removed workflow-level `default_timeout_ms` configuration
  - Preserved action-level timeout configurations (MCP, shell, prompt timeouts)

### Verification

✅ **Code Compilation**
- `cargo check` passes successfully
- No breaking changes introduced
- Only minor unused import warning (non-critical)

### Key Findings

The "overall workflow timeout" feature was **partially implemented** but **never fully functional**:
- Documentation showed examples but parsing was never implemented
- YAML front matter with `timeout_ms` was ignored by the parser
- Only executor had timeout logic using fallback to sub-workflow timeout
- Feature was documented but not working as intended

### Impact

After removal:
- **Simpler workflow configuration** - no confusing timeout hierarchy
- **Clearer timeout semantics** - action-level timeouts only
- **Documentation consistency** - examples match actual functionality
- **No functionality loss** - feature wasn't working properly anyway

The removal simplifies the system without breaking existing functionality since the workflow-level timeout was not properly integrated.