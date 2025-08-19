# Update MCP Server Integration for New Storage Defaults

## Overview
Update MCP server integration in `swissarmyhammer-tools/src/mcp/server.rs` to use the new storage default behavior instead of hardcoded paths.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
- **File**: `swissarmyhammer-tools/src/mcp/server.rs:118`
- **Current Logic**: `work_dir.join("issues")`
- **Issue**: Hardcoded path doesn't follow new directory structure

## Target Implementation

### Update MCP Server Initialization
```rust
// Replace work_dir.join("issues") with:
let issues_storage = {
    let original_dir = std::env::current_dir()?;
    if work_dir != original_dir {
        std::env::set_current_dir(&work_dir)?;
    }
    
    let storage = swissarmyhammer::issues::FileSystemIssueStorage::new_default()?;
    
    if work_dir != original_dir {
        std::env::set_current_dir(original_dir)?;
    }
    
    storage
};
```

### Alternative Context-Aware Implementation
If direct directory specification is needed:
```rust
// Create a context-aware storage constructor
let issues_dir = if work_dir.join(".swissarmyhammer").exists() {
    work_dir.join(".swissarmyhammer").join("issues")
} else {
    work_dir.join("issues") // Backward compatibility
};
let issues_storage = swissarmyhammer::issues::FileSystemIssueStorage::new(issues_dir)?;
```

### Update MCP Tool Context
```rust
// Ensure MCP tool context uses the updated storage
let tool_context = SwissArmyHammerToolContext::new(
    work_dir.clone(),
    prompt_library,
    workflow_library,
    Arc::new(RwLock::new(Box::new(issues_storage))),
    Arc::new(RwLock::new(Box::new(memo_storage))),
)?;
```

## Implementation Details

### Working Directory Context
- Handle MCP server working directory different from current directory
- Maintain working directory context for storage operations
- Ensure thread safety for directory changes

### MCP Protocol Compatibility
- Maintain MCP protocol message handling
- Ensure tool responses include correct paths
- Verify MCP client compatibility with new paths

### Storage Configuration
- Use consistent storage configuration across MCP tools
- Handle storage initialization errors properly
- Maintain connection lifecycle management

### Error Handling
- Handle working directory change errors
- Provide meaningful error messages for MCP clients
- Log configuration decisions for debugging

## Testing Requirements

### MCP Integration Tests
- Test MCP server startup with new directory structure
- Test MCP tool operations with new storage paths
- Test backward compatibility with legacy directory structure
- Test MCP error handling for storage failures

### MCP Protocol Tests
- Verify MCP protocol messages include correct paths
- Test MCP tool execution with new storage
- Test concurrent MCP client operations
- Test MCP server lifecycle management

### Cross-Platform Tests
- Test directory handling on different platforms
- Test path resolution with various working directories
- Test permission handling for directory access

## Files to Modify
- `swissarmyhammer-tools/src/mcp/server.rs`
- Update MCP server integration tests
- Update any MCP-specific documentation
- Review MCP tool implementations for path assumptions

## Integration with MCP Tools

### Tool Path Handling
Review and update MCP tools that might assume specific issue paths:
- Issue creation tools
- Issue listing tools
- Issue management tools
- File path validation in tools

### Response Path Consistency
Ensure MCP tool responses return correct paths:
- File paths in tool responses
- Error messages with paths
- Status information with locations

## Acceptance Criteria
- [ ] MCP server uses new storage defaults instead of hardcoded paths
- [ ] MCP tools work correctly with new directory structure
- [ ] Backward compatibility maintained for existing MCP clients
- [ ] All MCP integration tests pass
- [ ] MCP protocol compatibility maintained
- [ ] Working directory context handled properly
- [ ] Thread safety maintained for concurrent MCP operations
- [ ] Error handling provides meaningful messages

## Dependencies
- Depends on step 000284 (core storage update)
- Depends on step 000285 (migration detection)
- Can be done in parallel with step 000286 (CLI integration)

## Estimated Effort
~200-250 lines of code changes including tests and MCP tool review.

## Notes
- Pay attention to MCP protocol message paths
- Verify compatibility with existing MCP clients
- Test with different working directory scenarios
- Consider impact on MCP tool registration and discovery