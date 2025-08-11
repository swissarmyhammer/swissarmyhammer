# ABORT_000258: Built-in Prompt Updates for Abort Tool

Refer to ./specification/abort.md

## Objective

Update `builtin/prompts/abort.md` to use the new MCP abort tool instead of instructing users to respond with "ABORT ERROR" strings. This modernizes the abort prompt to use the robust file-based system.

## Context

The current abort prompt at `builtin/prompts/abort.md:9` contains the instruction `"Respond only with ABORT ERROR"` which relies on the brittle string-based detection system. This needs to be updated to provide clear instructions for using the new MCP abort tool.

## Current State Analysis

Location: `builtin/prompts/abort.md`

The current prompt instructs users to output "ABORT ERROR" strings, which will no longer work with the new file-based abort system.

## Tasks

### 1. Update Abort Prompt Content

**Update: `builtin/prompts/abort.md`**

Replace the current content with instructions for the MCP abort tool:

```markdown
---
description: "Signal immediate termination of the current workflow or operation"
usage: "Use when you need to abort execution due to safety, validation, or user request"
arguments: []
tags: ["control", "safety", "termination"]
version: "2.0.0"
---

# Abort Execution

Use the abort MCP tool to immediately terminate the current workflow or operation. This creates an abort signal that will be detected by the execution system and cause immediate termination.

## When to Use

- Safety concerns require immediate termination
- Pre-condition validation fails and continuing would be unsafe  
- User explicitly requests cancellation of a destructive operation
- Critical errors that cannot be recovered from
- Resource constraints prevent safe continuation

## Usage

Use the abort MCP tool with a clear reason for the termination:

```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Clear description of why execution is being aborted"
  }
}
```

## Examples

### Safety Abort
```json
{
  "tool": "abort", 
  "parameters": {
    "reason": "Destructive operation cancelled - user safety confirmation required"
  }
}
```

### Validation Failure
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Pre-condition validation failed: critical files missing"
  }
}
```

### User Request
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "User requested cancellation of long-running operation"
  }
}
```

### Resource Constraints
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Insufficient disk space to complete operation safely"
  }
}
```

## Behavior

- Creates `.swissarmyhammer/.abort` file with the specified reason
- Workflow execution will terminate at the next check point
- Error propagation maintains the abort reason for debugging
- Exit codes indicate aborted execution (not failure)
- Logging captures abort reason for troubleshooting

## Best Practices

- **Be Specific**: Provide clear, actionable abort reasons
- **Safety First**: Use abort for any safety-critical situations
- **User Communication**: Include user-facing explanations when appropriate
- **Context Preservation**: Include relevant context in the abort reason
- **Graceful Termination**: Allow abort to provide clean termination

## Migration from Legacy Format

**Old Format (Deprecated):**
```
Respond only with ABORT ERROR
```

**New Format (Recommended):**
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Specific reason for abort"
  }
}
```

The old string-based format is deprecated and will be removed in future versions. Use the MCP tool format for reliable abort handling.
```

### 2. Update Related Prompts That Reference Abort

Search for other prompts that might reference the old abort mechanism:

**Command to find references:**
```bash
grep -r "ABORT ERROR" builtin/prompts/ --exclude="abort.md"
```

Update any found references to use the new MCP tool format.

### 3. Add Abort Tool Usage Examples

**Create: `builtin/prompts/examples/abort_scenarios.md`**

```markdown
---
description: "Example scenarios for using the abort MCP tool"
usage: "Reference examples for different abort situations"
tags: ["examples", "abort", "safety"]
---

# Abort Tool Usage Scenarios

## Scenario 1: File Operation Safety Check

```markdown
I need to analyze the request to delete all files in the project directory.

This operation could be destructive and irreversible. For safety, I should abort this workflow until the user confirms their intent.

{
  "tool": "abort",
  "parameters": {
    "reason": "Destructive file deletion requested - requires explicit user confirmation for safety"
  }
}
```

## Scenario 2: Missing Dependencies

```markdown  
I'm attempting to run tests but the required test framework is not installed.

{
  "tool": "abort",
  "parameters": {
    "reason": "Required test dependencies missing: pytest not found in environment"
  }
}
```

## Scenario 3: Invalid Configuration

```markdown
The configuration file contains invalid settings that would cause system failure.

{
  "tool": "abort", 
  "parameters": {
    "reason": "Configuration validation failed: invalid database connection string"
  }
}
```

## Scenario 4: User Cancellation

```markdown
The user has indicated they want to cancel the current long-running operation.

{
  "tool": "abort",
  "parameters": {
    "reason": "User requested cancellation of data migration process"
  }
}
```

## Scenario 5: Resource Exhaustion

```markdown
System resources are insufficient to complete the requested operation safely.

{
  "tool": "abort",
  "parameters": {
    "reason": "Insufficient memory available for large dataset processing - requires 8GB, only 2GB free"
  }
}
```
```

### 4. Update Documentation References

**Check and update these documentation files if they reference abort:**
- `doc/src/workflows.md` - Update abort error handling section
- Any workflow examples that show abort usage
- Built-in workflow files in `builtin/workflows/`

### 5. Add Validation for Prompt Changes

**Create: `builtin/prompts/abort_validation_test.md`**

```markdown
---
description: "Test prompt for validating abort tool functionality"  
usage: "Internal validation of abort tool behavior"
tags: ["test", "validation", "abort"]
---

# Abort Tool Validation Test

This prompt tests the abort tool functionality to ensure it works correctly.

Step 1: Test basic abort functionality
{
  "tool": "abort",
  "parameters": {
    "reason": "Validation test - abort tool functionality check"
  }
}

This should create an abort file and terminate execution immediately.
```

### 6. Update Built-in Workflow References

Check built-in workflows for abort usage:

**Search command:**
```bash
grep -r "ABORT ERROR" builtin/workflows/
```

Update any workflows that reference the old abort mechanism to use conditional logic or the new abort tool.

### 7. Add Prompt Version Migration

**Create: `builtin/prompts/migration/abort_migration_guide.md`**

```markdown
---
description: "Migration guide for updating custom prompts to use new abort tool"
usage: "Guide for users updating their custom prompts"
tags: ["migration", "guide", "abort"]  
---

# Abort Tool Migration Guide

## Overview

The abort mechanism has been upgraded from string-based detection to a robust MCP tool-based system. This guide helps migrate existing prompts.

## Migration Steps

### 1. Identify Old Abort Usage

Find prompts containing:
- `Respond only with ABORT ERROR`
- `Return ABORT ERROR`
- Any string-based abort instructions

### 2. Replace with MCP Tool Usage

**Before:**
```markdown
If the operation is unsafe, respond only with ABORT ERROR.
```

**After:**
```markdown
If the operation is unsafe, use the abort tool:

{
  "tool": "abort",
  "parameters": {
    "reason": "Operation unsafe - [specific reason]"
  }
}
```

### 3. Add Context to Abort Reasons

Provide specific, actionable information in abort reasons:

**Good:**
```json
{
  "reason": "Database connection failed - unable to verify user permissions before proceeding"
}
```

**Poor:**
```json
{
  "reason": "Something went wrong"
}
```

## Benefits of New System

- **Reliability**: File-based detection is more robust
- **Debugging**: Abort reasons are preserved in logs
- **Context**: Rich information available for troubleshooting
- **Safety**: Atomic operations prevent race conditions
- **Consistency**: Unified error handling across all components
```

## Success Criteria

- [ ] `builtin/prompts/abort.md` updated with MCP tool instructions
- [ ] All references to "ABORT ERROR" strings removed from built-in prompts
- [ ] Comprehensive examples provided for different abort scenarios
- [ ] Documentation updated to reflect new abort mechanism
- [ ] Built-in workflows updated to use new abort system
- [ ] Migration guide available for custom prompt updates
- [ ] Validation test confirms abort tool functionality
- [ ] Version information updated in prompt metadata

## Testing

```bash
# Validate prompt syntax and content
cargo run -- prompt validate builtin/prompts/abort.md

# Test abort tool functionality through prompt
cargo run -- prompt test builtin/prompts/abort_validation_test.md

# Search for remaining string-based abort references
grep -r "ABORT ERROR" builtin/

# Validate all built-in prompts still work
cargo run -- prompt validate --all

# Run prompt integration tests
cargo test prompt_integration
```

## Notes

- Maintains backward compatibility during transition by documenting both formats
- Provides comprehensive examples to guide proper usage
- Includes specific migration instructions for existing custom prompts
- Emphasizes safety and clarity in abort reason messages
- Links to broader documentation updates in workflow and error handling sections

## Next Steps

After completion, proceed to ABORT_000259 for comprehensive testing of the abort system.