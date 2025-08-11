# Update Documentation for New Abort System

Refer to ./specification/abort.md

## Objective
Update all documentation to reflect the new file-based abort system, including workflow documentation, error handling patterns, and user-facing documentation, while removing references to the old string-based system.

## Context
The documentation currently describes and references the old string-based "ABORT ERROR" system. All documentation must be updated to accurately describe the new MCP tool-based abort system and provide clear guidance for users and developers.

## Tasks

### 1. Update Workflow Documentation
Location: `doc/src/workflows.md`
- Update abort error handling section to describe file-based system
- Remove references to "ABORT ERROR" string patterns
- Add documentation for abort MCP tool usage
- Update examples to show proper abort tool usage

### 2. Update Error Handling Patterns Memo
Location: `.swissarmyhammer/memos/Error Handling and Resilience Patterns.md`
- Remove references to string-based ABORT ERROR pattern
- Add documentation for file-based abort system
- Update error propagation examples
- Document ExecutorError::Abort variant

### 3. Update Generated Documentation
Location: `doc/book/print.html`, `doc/book/workflows.html`
- Regenerate documentation with updated content
- Ensure all HTML documentation reflects new abort system
- Update search index if needed
- Verify all links and references work correctly

### 4. Create MCP Tool Documentation
Create comprehensive documentation for the abort MCP tool:
- Tool purpose and usage
- Parameter descriptions and examples
- Integration with workflow system
- Best practices for abort usage

### 5. Update README and High-Level Documentation
- Update any README files that reference abort functionality
- Update getting started guides if they mention abort
- Update troubleshooting guides with new abort system info

### 6. Search and Update All Documentation References
Use comprehensive search to find and update:
- All "ABORT ERROR" references in documentation
- Examples using old abort system
- Error handling documentation
- Workflow examples and tutorials

## Implementation Details

### Workflow Documentation Updates
```markdown
## Error Handling and Abort Conditions

### Abort Tool Usage

To abort a workflow, action, or process, use the `abort` MCP tool:

```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Clear description of why abort was necessary"
  }
}
```

### How Abort Works

1. The abort tool creates a `.swissarmyhammer/.abort` file with the reason
2. The workflow executor detects this file and terminates execution
3. An `ExecutorError::Abort` is raised with the abort reason
4. The CLI handles the error and exits with appropriate code

### Abort File Cleanup

Abort files are automatically cleaned up when:
- A new workflow run is started via `WorkflowRun::new()`
- The workflow completes successfully
```

### MCP Tool Documentation
```markdown
# Abort Tool

The abort tool provides controlled termination of workflows, actions, and processes.

## Parameters

- `reason` (required): String describing why the abort was necessary

## Usage Examples

### User Cancellation
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "User cancelled the operation"
  }
}
```

### Unsafe Conditions
```json
{
  "tool": "abort", 
  "parameters": {
    "reason": "Detected potentially unsafe file system operation"
  }
}
```

## Integration

The abort tool integrates with the workflow system by:
1. Creating a `.swissarmyhammer/.abort` file
2. Being detected by the workflow executor
3. Causing immediate workflow termination
```

### Error Handling Pattern Updates
Remove old ABORT ERROR patterns and add:
```markdown
## File-Based Abort Pattern

**Critical Failure Pattern**
```rust
// Modern abort detection using file-based approach
if std::path::Path::new(".swissarmyhammer/.abort").exists() {
    let reason = std::fs::read_to_string(".swissarmyhammer/.abort")
        .unwrap_or_else(|_| "Unknown abort reason".to_string());
    return Err(ExecutorError::Abort(reason));
}
```

**Use Cases for Abort Tool**
- User-initiated cancellation
- Safety violations detected
- Prerequisites cannot be met
- System consistency violations
```

## Validation Criteria
- [ ] All documentation references new abort system
- [ ] No "ABORT ERROR" string references remain in docs
- [ ] MCP tool documentation is comprehensive
- [ ] Examples show correct abort tool usage
- [ ] Error handling documentation is accurate
- [ ] Generated documentation is updated
- [ ] Documentation build succeeds without warnings
- [ ] All links and references work correctly

## Documentation Files to Update
Based on specification analysis:
- `doc/src/workflows.md`
- `doc/book/print.html` (regenerated)
- `doc/book/workflows.html` (regenerated)
- `doc/book/searchindex.js` (regenerated)
- `.swissarmyhammer/memos/Error Handling and Resilience Patterns.md`
- Any README files with abort references
- Any other documentation found through search

## Dependencies
- ABORT_000267_test-suite-updates (implementation must be complete)
- ABORT_000264_builtin-prompt-updates (prompts must be updated)

## Follow-up Issues
- ABORT_000269_final-integration-testing