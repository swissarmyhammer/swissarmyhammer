# Implement Core Abort MCP Tool Functionality

Refer to ./specification/abort.md

## Objective
Implement the core abort MCP tool that creates the `.swissarmyhammer/.abort` file with the abort reason, providing atomic file-based abort state management.

## Context
Following the established MCP tool pattern, implement the actual abort functionality that will replace the brittle string-based "ABORT ERROR" detection. The tool must be robust, atomic, and work across process boundaries.

## Tasks

### 1. Core Tool Implementation
Implement the `abort_create` tool in `tools/abort/create/mod.rs`:
- Accept required `reason` parameter
- Create `.swissarmyhammer/.abort` file with atomic operations
- Handle file creation errors gracefully
- Return success to allow proper error propagation

### 2. File-Based State Management
- Use `.swissarmyhammer/.abort` file location for consistency
- Write plain text abort reason to file
- Ensure atomic file creation (create temp file, then rename)
- Handle race conditions and concurrent access

### 3. Error Handling
- Proper error handling for file system operations
- Graceful handling of directory creation if needed
- Clear error messages for debugging
- Tool should not fail - errors should be logged but tool returns success

### 4. Tool Description
Create `tools/abort/create/description.md` with:
- Clear documentation of tool purpose
- Parameter descriptions
- Usage examples
- Integration notes

## Implementation Details

### Tool Logic
```rust
pub async fn execute(&self, arguments: Value) -> Result<Value, Box<dyn std::error::Error>> {
    let params: AbortCreateParameters = serde_json::from_value(arguments)?;
    
    // Ensure .swissarmyhammer directory exists
    std::fs::create_dir_all(".swissarmyhammer")?;
    
    // Write abort file atomically
    let temp_path = ".swissarmyhammer/.abort.tmp";
    let final_path = ".swissarmyhammer/.abort";
    
    std::fs::write(temp_path, &params.reason)?;
    std::fs::rename(temp_path, final_path)?;
    
    Ok(json!({
        "success": true,
        "message": format!("Abort initiated: {}", params.reason)
    }))
}
```

### File Format
- Plain text file containing the abort reason
- No additional metadata initially (can be extended later)
- UTF-8 encoding for proper text handling

## Validation Criteria
- [ ] Tool creates `.swissarmyhammer/.abort` file successfully
- [ ] File contains the provided reason text
- [ ] File creation is atomic (no partial writes)
- [ ] Directory is created if it doesn't exist
- [ ] Tool handles file system errors gracefully
- [ ] Tool returns success even if file creation fails (for error propagation)
- [ ] Description.md provides clear usage instructions

## Testing Requirements
- Unit tests for file creation functionality
- Tests for atomic file operations
- Error handling tests for file system failures
- Concurrent access testing

## Dependencies
- ABORT_000259_mcp-tool-infrastructure (must be completed first)

## Follow-up Issues
- ABORT_000261_workflowrun-cleanup-integration