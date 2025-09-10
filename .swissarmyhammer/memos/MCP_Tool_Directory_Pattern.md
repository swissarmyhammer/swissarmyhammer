# MCP Tool Directory Pattern

## Directory Structure

Each MCP tool follows a consistent directory structure:

```
src/mcp/tools/{category}/{tool_name}/
├── mod.rs           # Main implementation
├── description.md   # Tool description for MCP
├── tests.rs         # Unit tests (optional)
└── examples/        # Usage examples (optional)
```

## Implementation Pattern

### Tool Registration
- Tools are registered in `tool_registry.rs`
- Each tool has a unique name following snake_case convention
- Tools are grouped by category (files, shell, memoranda, etc.)

### Handler Pattern
- All tools implement the same handler signature
- Tools receive JSON parameters and return structured responses
- Error handling follows the repository error patterns
- Use `serde_json` for parameter parsing and response serialization

### Parameter Validation
- Validate all required parameters early
- Use type-safe parameter structures with `serde` derives
- Provide clear error messages for invalid parameters
- Use `Option<T>` for optional parameters

### Response Format
- Return consistent response structure
- Include success/error status
- Provide meaningful error messages
- Use structured data for complex responses

## Tool Categories

### File Operations
- `files_read`: Read file contents
- `files_write`: Write file contents
- `files_edit`: Edit file with find/replace
- `files_glob`: Find files matching patterns
- `files_grep`: Search file contents

### Shell Operations
- `shell_execute`: Execute shell commands
- Focus on safety and timeout handling
- Capture stdout, stderr, and exit codes
- Support working directory and environment variables

### Search Operations
- `search_index`: Index files for semantic search
- `search_query`: Perform semantic searches
- Use vector embeddings for code understanding
- Support multiple file types and languages

### Issue Management
- `issue_create`: Create new issues
- `issue_list`: List existing issues
- `issue_show`: Display issue details
- `issue_work`: Switch to issue branch
- `issue_mark_complete`: Mark issues as done

## Implementation Guidelines

### Error Handling
- Use the common error types from `swissarmyhammer-common`
- Wrap external errors with context
- Return user-friendly error messages
- Log errors at appropriate levels

### Testing
- Write unit tests for each tool
- Test parameter validation
- Test error conditions
- Use integration tests for complex workflows

### Documentation
- Maintain `description.md` for each tool
- Include parameter descriptions
- Provide usage examples
- Keep documentation synchronized with implementation