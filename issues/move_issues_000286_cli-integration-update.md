# Update CLI Integration to Use New Storage Defaults

## Overview
Update CLI integration in `swissarmyhammer-cli/src/mcp_integration.rs` to use the updated storage defaults instead of hardcoded paths.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
- **File**: `swissarmyhammer-cli/src/mcp_integration.rs:60-64`
- **Method**: `CliToolContext::create_issue_storage()`
- **Current Logic**: `current_dir.join("issues")`
- **Issue**: Hardcoded path ignores new default behavior

## Target Implementation

### Update CLI Storage Creation
```rust
fn create_issue_storage(
    current_dir: &std::path::Path,
) -> Result<IssueStorageArc, Box<dyn std::error::Error>> {
    // Use the updated new_default() method instead of hardcoded path
    Ok(Arc::new(RwLock::new(Box::new(
        swissarmyhammer::issues::FileSystemIssueStorage::new_default()?,
    ))))
}
```

### Alternative Implementation (if needed)
If directory context is still needed:
```rust
fn create_issue_storage(
    current_dir: &std::path::Path,
) -> Result<IssueStorageArc, Box<dyn std::error::Error>> {
    // Temporarily change to the specified directory for default resolution
    let original_dir = std::env::current_dir()?;
    if current_dir != original_dir {
        std::env::set_current_dir(current_dir)?;
    }
    
    let storage = swissarmyhammer::issues::FileSystemIssueStorage::new_default()?;
    
    // Restore original directory
    if current_dir != original_dir {
        std::env::set_current_dir(original_dir)?;
    }
    
    Ok(Arc::new(RwLock::new(Box::new(storage))))
}
```

### Update Related Functions
Review and update any other CLI functions that might have hardcoded issue directory paths:
```rust
// Check for similar patterns in CLI integration
// Update any hardcoded "./issues" references to use storage defaults
```

## Implementation Details

### Code Analysis
1. Review `mcp_integration.rs` for all hardcoded issue directory references
2. Identify any other functions that might need updates
3. Ensure consistent behavior across all CLI operations

### Storage Context Handling
- Determine if `current_dir` parameter is still needed
- Handle directory context changes if required
- Ensure thread safety for concurrent operations

### Error Handling
- Maintain existing error handling patterns
- Add appropriate error context for storage creation failures
- Handle directory change errors gracefully

### Testing Integration
- Verify CLI tests work with new storage behavior
- Update test helpers if needed
- Ensure test isolation continues to work

## Testing Requirements

### CLI Integration Tests
- Test CLI commands with new directory structure
- Test backward compatibility with legacy directory structure
- Test CLI behavior when both directories exist
- Test CLI error handling for storage creation failures

### Unit Tests
- Test `create_issue_storage()` function directly
- Test directory context handling
- Test error propagation
- Test thread safety of storage creation

### End-to-End Tests
- Test complete CLI workflows with new storage
- Test issue creation, listing, and management
- Test CLI behavior in various directory scenarios

## Files to Modify
- `swissarmyhammer-cli/src/mcp_integration.rs`
- Update CLI integration tests
- Update any CLI-specific documentation

## Acceptance Criteria
- [ ] CLI uses new storage defaults instead of hardcoded paths
- [ ] Backward compatibility maintained for existing CLI usage
- [ ] All CLI tests pass with new storage behavior
- [ ] Error handling properly propagates storage creation errors
- [ ] Thread safety maintained for concurrent CLI operations
- [ ] No breaking changes to CLI public interface
- [ ] Performance regression avoided

## Dependencies
- Depends on step 000284 (core storage update)
- Depends on step 000285 (migration detection) for complete functionality

## Estimated Effort
~100-150 lines of code changes including tests and verification.

## Notes
- This step primarily removes hardcoded paths rather than adding new functionality
- Focus on maintaining existing CLI behavior while using improved storage defaults
- Ensure CLI integration tests verify the behavior works correctly