# Grep Tool Implementation

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Implement the Grep tool for content-based search using ripgrep for fast and flexible text searching.

## Tool Specification
**Parameters**:
- `pattern` (required): Regular expression pattern to search
- `path` (optional): File or directory to search in
- `glob` (optional): Glob pattern to filter files (e.g., `*.js`)
- `type` (optional): File type filter (e.g., `js`, `py`, `rust`)
- `case_insensitive` (optional): Case-insensitive search
- `context_lines` (optional): Number of context lines around matches
- `output_mode` (optional): Output format (`content`, `files_with_matches`, `count`)

## Tasks
- [ ] Create `GrepTool` struct implementing `McpTool` trait
- [ ] Implement ripgrep integration for high-performance search
- [ ] Add regex pattern validation and compilation
- [ ] Implement file type filtering and glob pattern support
- [ ] Add context line extraction functionality
- [ ] Implement multiple output modes (content, files, count)
- [ ] Add integration with security validation framework
- [ ] Create tool description in `description.md`
- [ ] Implement JSON schema for parameter validation

## Implementation Details
```rust
// In files/grep/mod.rs
pub struct GrepTool;

impl McpTool for GrepTool {
    fn name(&self) -> &'static str { "file_grep" }
    fn schema(&self) -> serde_json::Value { /* schema definition */ }
    async fn execute(&self, arguments: serde_json::Value, context: ToolContext) -> Result<CallToolResult>;
}

// Key functionality
- search_with_ripgrep(pattern: &str, options: GrepOptions) -> Result<GrepResults>
- validate_regex_pattern(pattern: &str) -> Result<regex::Regex>
- extract_context_lines(content: &str, match_line: usize, context: usize) -> (Vec<String>, Vec<String>)
- format_output(results: GrepResults, mode: OutputMode) -> Result<String>
```

## Functionality Requirements
- Leverages ripgrep for high-performance text search
- Supports full regular expression syntax
- Provides file type and glob filtering
- Returns contextual information around matches
- Handles large codebases efficiently
- Multiple output formats for different use cases

## Use Cases Covered
- Finding function definitions or usages
- Searching for specific code patterns
- Locating configuration values
- Identifying potential issues or code smells

## Testing Requirements
- [ ] Unit tests for regex pattern validation
- [ ] Tests for various output modes
- [ ] File type filtering tests
- [ ] Glob pattern integration tests
- [ ] Context line extraction tests
- [ ] Performance tests with large codebases
- [ ] Case sensitivity option tests
- [ ] Security validation integration tests
- [ ] Error handling tests (invalid regex, permission issues)

## Acceptance Criteria
- [ ] Tool fully implements MCP Tool trait
- [ ] Ripgrep integration for high performance
- [ ] Full regex pattern support with validation
- [ ] Multiple output modes implemented
- [ ] File type and glob filtering functionality
- [ ] Context line extraction capability
- [ ] Integration with security validation framework
- [ ] Complete test coverage including edge cases
- [ ] Tool registration in module system
- [ ] Performance benchmarks showing efficient operation