# Glob Tool Implementation

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Implement the Glob tool for fast file pattern matching with advanced filtering and sorting capabilities.

## Tool Specification
**Parameters**:
- `pattern` (required): Glob pattern to match files (e.g., `**/*.js`, `src/**/*.ts`)
- `path` (optional): Directory to search within
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore patterns (default: true)

## Tasks
- [ ] Create `GlobTool` struct implementing `McpTool` trait
- [ ] Implement glob pattern matching using `glob` crate
- [ ] Add integration with `ignore` crate for .gitignore support
- [ ] Implement sorting by modification time (recent first)
- [ ] Add workspace boundary validation for search paths
- [ ] Add case sensitivity handling
- [ ] Add integration with security validation framework
- [ ] Create tool description in `description.md`
- [ ] Implement JSON schema for parameter validation

## Implementation Details
```rust
// In files/glob/mod.rs
pub struct GlobTool;

impl McpTool for GlobTool {
    fn name(&self) -> &'static str { "file_glob" }
    fn schema(&self) -> serde_json::Value { /* schema definition */ }
    async fn execute(&self, arguments: serde_json::Value, context: ToolContext) -> Result<CallToolResult>;
}

// Key functionality
- find_files_by_pattern(pattern: &str, base_path: Option<&Path>, options: GlobOptions) -> Result<Vec<PathBuf>>
- apply_gitignore_filtering(files: Vec<PathBuf>, base_path: &Path) -> Result<Vec<PathBuf>>
- sort_by_modification_time(files: &mut Vec<PathBuf>) -> Result<()>
- validate_glob_pattern(pattern: &str) -> Result<()>
```

## Functionality Requirements
- Supports standard glob patterns with wildcards (`*`, `**`, `?`, `[...]`)
- Returns file paths sorted by modification time (recent first)
- Searches within specified directory or entire workspace
- Respects git ignore patterns and workspace boundaries
- Provides fast pattern matching for large codebases
- Case-sensitive/insensitive matching support

## Use Cases Covered
- Finding files by name patterns
- Locating specific file types
- Discovering recently modified files
- Building file lists for batch operations

## Testing Requirements
- [ ] Unit tests for various glob patterns (`*`, `**`, `?`, character classes)
- [ ] Tests for modification time sorting
- [ ] .gitignore integration tests
- [ ] Case sensitivity option tests
- [ ] Workspace boundary validation tests
- [ ] Performance tests with large codebases
- [ ] Security validation integration tests
- [ ] Error handling tests (invalid patterns, permission issues)

## Acceptance Criteria
- [ ] Tool fully implements MCP Tool trait
- [ ] Comprehensive glob pattern support
- [ ] Integration with ignore crate for .gitignore support
- [ ] Modification time sorting implemented
- [ ] Integration with security validation framework
- [ ] Complete test coverage including edge cases
- [ ] Tool registration in module system
- [ ] Performance optimized for large directory trees