# Read Tool Implementation

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Implement the Read tool for reading file contents with support for partial reading and multiple file types.

## Tool Specification
**Parameters**:
- `absolute_path` (required): Full absolute path to the file
- `offset` (optional): Starting line number for partial reading
- `limit` (optional): Maximum number of lines to read

## Tasks
- [ ] Create `ReadTool` struct implementing `McpTool` trait
- [ ] Implement file reading with offset/limit support
- [ ] Add support for text files, images, PDFs, and other file types
- [ ] Implement comprehensive error handling for missing/inaccessible files
- [ ] Add integration with security validation framework
- [ ] Create tool description in `description.md`
- [ ] Implement JSON schema for parameter validation

## Implementation Details
```rust
// In files/read/mod.rs
pub struct ReadTool;

impl McpTool for ReadTool {
    fn name(&self) -> &'static str { "file_read" }
    fn schema(&self) -> serde_json::Value { /* schema definition */ }
    async fn execute(&self, arguments: serde_json::Value, context: ToolContext) -> Result<CallToolResult>;
}

// Key functionality
- read_file_with_limits(path: &Path, offset: Option<usize>, limit: Option<usize>) -> Result<String>
- detect_file_type(path: &Path) -> FileType
- read_binary_file_as_base64(path: &Path) -> Result<String>
```

## Use Cases Covered
- Reading source code files for analysis
- Examining configuration files
- Viewing documentation or README files
- Reading specific sections of large files
- Handling binary files (images, PDFs) with appropriate encoding

## Testing Requirements
- [ ] Unit tests for file reading with various parameters
- [ ] Tests for offset/limit functionality
- [ ] Error handling tests (missing files, permission issues)
- [ ] Binary file handling tests
- [ ] Security validation integration tests
- [ ] Performance tests with large files

## Acceptance Criteria
- [ ] Tool fully implements MCP Tool trait
- [ ] Comprehensive parameter validation via JSON schema
- [ ] Support for all specified file types
- [ ] Integration with security validation framework
- [ ] Complete test coverage including edge cases
- [ ] Tool registration in module system