# Write Tool Implementation

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Implement the Write tool for creating new files or completely overwriting existing files with atomic operations.

## Tool Specification
**Parameters**:
- `file_path` (required): Absolute path for the new or existing file
- `content` (required): Complete file content to write

## Tasks
- [ ] Create `WriteTool` struct implementing `McpTool` trait
- [ ] Implement atomic file writing with temporary file strategy
- [ ] Add parent directory creation functionality
- [ ] Implement content validation and encoding handling
- [ ] Add integration with security validation framework
- [ ] Create tool description in `description.md`
- [ ] Implement JSON schema for parameter validation

## Implementation Details
```rust
// In files/write/mod.rs
pub struct WriteTool;

impl McpTool for WriteTool {
    fn name(&self) -> &'static str { "file_write" }
    fn schema(&self) -> serde_json::Value { /* schema definition */ }
    async fn execute(&self, arguments: serde_json::Value, context: ToolContext) -> Result<CallToolResult>;
}

// Key functionality
- write_file_atomic(path: &Path, content: &str) -> Result<()>
- create_parent_directories(path: &Path) -> Result<()>
- validate_content_encoding(content: &str) -> Result<()>
- set_appropriate_permissions(path: &Path) -> Result<()>
```

## Functionality Requirements
- Creates new files with specified content
- Overwrites existing files completely (with user confirmation in description)
- Creates parent directories if they don't exist
- Sets appropriate file permissions
- Uses atomic write operations (write to temp file, then rename)
- Validates file path and content encoding

## Use Cases Covered
- Creating new source files
- Generating configuration files
- Writing documentation or README files
- Creating test files or fixtures

## Testing Requirements
- [ ] Unit tests for file creation and overwriting
- [ ] Tests for parent directory creation
- [ ] Atomic operation tests (interruption scenarios)
- [ ] Content encoding validation tests
- [ ] Security validation integration tests
- [ ] Permission handling tests
- [ ] Error handling tests (disk space, permissions, etc.)

## Acceptance Criteria
- [ ] Tool fully implements MCP Tool trait
- [ ] Atomic write operations implemented
- [ ] Parent directory creation functionality
- [ ] Integration with security validation framework
- [ ] Complete test coverage including edge cases
- [ ] Tool registration in module system
- [ ] Proper error handling for all failure modes