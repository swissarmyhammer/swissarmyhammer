# Edit Tool Implementation

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Implement the Edit tool for performing precise string replacements in existing files with atomic operations.

## Tool Specification
**Parameters**:
- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences (default: false)

## Tasks
- [ ] Create `EditTool` struct implementing `McpTool` trait
- [ ] Implement exact string matching and replacement logic
- [ ] Add validation for old_string existence and uniqueness
- [ ] Implement atomic edit operations using temporary files
- [ ] Add file encoding and line ending preservation
- [ ] Add integration with security validation framework
- [ ] Create tool description in `description.md`
- [ ] Implement JSON schema for parameter validation

## Implementation Details
```rust
// In files/edit/mod.rs
pub struct EditTool;

impl McpTool for EditTool {
    fn name(&self) -> &'static str { "file_edit" }
    fn schema(&self) -> serde_json::Value { /* schema definition */ }
    async fn execute(&self, arguments: serde_json::Value, context: ToolContext) -> Result<CallToolResult>;
}

// Key functionality
- find_and_replace_atomic(path: &Path, old: &str, new: &str, replace_all: bool) -> Result<EditResult>
- validate_old_string_exists(content: &str, old_string: &str) -> Result<usize>
- validate_old_string_unique(content: &str, old_string: &str) -> Result<()>
- preserve_file_metadata(original: &Path, temp: &Path) -> Result<()>
```

## Functionality Requirements
- Performs exact string matching and replacement
- Maintains file encoding and line endings
- Validates that old_string exists in file
- Validates that old_string is unique (unless replace_all is true)
- Provides atomic operations (all or nothing replacement)
- Preserves file permissions and metadata

## Use Cases Covered
- Modifying specific code sections
- Updating variable names or function signatures
- Fixing bugs with targeted changes
- Refactoring code with precise replacements

## Testing Requirements
- [ ] Unit tests for exact string replacement
- [ ] Tests for replace_all functionality
- [ ] Validation tests (old_string existence and uniqueness)
- [ ] Atomic operation tests (interruption scenarios)
- [ ] File metadata preservation tests
- [ ] Encoding and line ending preservation tests
- [ ] Security validation integration tests
- [ ] Error handling tests (file not found, permission issues)

## Acceptance Criteria
- [ ] Tool fully implements MCP Tool trait
- [ ] Exact string matching and replacement implemented
- [ ] Atomic edit operations with rollback capability
- [ ] Comprehensive validation of old_string parameter
- [ ] Integration with security validation framework
- [ ] Complete test coverage including edge cases
- [ ] Tool registration in module system
- [ ] File metadata and encoding preservation