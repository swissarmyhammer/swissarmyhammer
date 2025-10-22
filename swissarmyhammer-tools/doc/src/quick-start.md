# Quick Start

This guide walks you through your first tasks with SwissArmyHammer Tools.

## Setup

### 1. Install SwissArmyHammer

```bash
cargo install swissarmyhammer
```

### 2. Verify Installation

```bash
sah --version
```

### 3. Navigate to Your Project

```bash
cd /path/to/your/project
```

## Starting the MCP Server

### Stdio Mode (for Claude Desktop)

```bash
sah serve
```

The server runs in stdio mode, communicating via standard input/output.

### HTTP Mode (for testing)

```bash
sah serve --http --port 3000
```

The server exposes HTTP endpoints on port 3000.

## Your First Tasks

### Task 1: List Available Tools

When connected via an MCP client like Claude Desktop, you can ask:

> "What tools are available?"

Or programmatically list tools using the MCP protocol.

### Task 2: Create an Issue

Create your first work item:

**Ask Claude:**
> "Create an issue for adding user authentication"

This uses the `issue_create` tool:
```json
{
  "name": "add-user-authentication",
  "content": "# Add User Authentication\n\nImplement JWT-based authentication.\n\n## Requirements\n- Login endpoint\n- Token validation\n- Session management"
}
```

### Task 3: List Your Issues

**Ask Claude:**
> "Show me all active issues"

This uses the `issue_list` tool:
```json
{
  "show_active": true,
  "format": "table"
}
```

### Task 4: Search Your Code

First, index your codebase:

**Ask Claude:**
> "Index all Rust files for semantic search"

This uses the `search_index` tool:
```json
{
  "patterns": ["**/*.rs"]
}
```

Then search:

**Ask Claude:**
> "Find code that handles error cases"

This uses the `search_query` tool:
```json
{
  "query": "error handling",
  "limit": 10
}
```

### Task 5: Find Files by Pattern

**Ask Claude:**
> "Find all test files"

This uses the `files_glob` tool:
```json
{
  "pattern": "**/*test*.rs"
}
```

### Task 6: Search File Contents

**Ask Claude:**
> "Find all TODO comments in the code"

This uses the `files_grep` tool:
```json
{
  "pattern": "TODO:",
  "output_mode": "content"
}
```

## Working with Files

### Read a File

**Ask Claude:**
> "Show me the contents of src/main.rs"

This uses the `files_read` tool:
```json
{
  "path": "/path/to/project/src/main.rs"
}
```

### Edit a File

**Ask Claude:**
> "Change the DEBUG flag to false in config.rs"

This uses the `files_edit` tool:
```json
{
  "file_path": "/path/to/project/src/config.rs",
  "old_string": "const DEBUG: bool = true;",
  "new_string": "const DEBUG: bool = false;"
}
```

### Write a New File

**Ask Claude:**
> "Create a new module for authentication"

This uses the `files_write` tool:
```json
{
  "file_path": "/path/to/project/src/auth.rs",
  "content": "//! Authentication module\n\npub fn authenticate(token: &str) -> bool {\n    // TODO: Implement\n    false\n}"
}
```

## Code Understanding

### Generate Code Outline

**Ask Claude:**
> "Generate an outline of the codebase structure"

This uses the `outline_generate` tool:
```json
{
  "patterns": ["src/**/*.rs"]
}
```

### Check Code Quality

**Ask Claude:**
> "Check the code for quality issues"

This uses the `rules_check` tool:
```json
{
  "file_paths": ["src/**/*.rs"]
}
```

## Git Integration

### Track Changes

**Ask Claude:**
> "What files have changed on this branch?"

This uses the `git_changes` tool:
```json
{
  "branch": "feature/add-auth"
}
```

## Web Operations

### Fetch Web Content

**Ask Claude:**
> "Fetch the content from https://example.com/docs"

This uses the `web_fetch` tool:
```json
{
  "url": "https://example.com/docs"
}
```

### Search the Web

**Ask Claude:**
> "Search for Rust async programming tutorials"

This uses the `web_search` tool:
```json
{
  "query": "Rust async programming tutorial",
  "category": "it",
  "results_count": 10
}
```

## Workflow Automation

### Execute a Workflow

**Ask Claude:**
> "Execute the test workflow"

This uses the `flow` tool:
```json
{
  "flow_name": "test"
}
```

## Common Patterns

### Pattern 1: Issue-Driven Development

1. Create issue: `issue_create`
2. Work on code: `files_read`, `files_edit`, `files_write`
3. Update issue with progress: `issue_update`
4. Complete issue: `issue_mark_complete`

### Pattern 2: Code Exploration

1. Index codebase: `search_index`
2. Search for relevant code: `search_query`
3. Read specific files: `files_read`
4. Generate outline: `outline_generate`

### Pattern 3: Quality Assurance

1. Check code quality: `rules_check`
2. Search for issues: `files_grep`
3. Fix issues: `files_edit`
4. Verify changes: `git_changes`

### Pattern 4: Documentation Research

1. Search web: `web_search`
2. Fetch specific pages: `web_fetch`
3. Save as memo: `memo_create`
4. Reference later: `memo_get`

## Tips for Success

### Working with Claude Desktop

1. **Be Specific**: Clearly describe what you want
2. **Provide Context**: Give file paths and details
3. **Review Changes**: Always review file modifications
4. **Use Issues**: Track complex work with issues

### Best Practices

1. **Index Early**: Run `search_index` when starting work
2. **Create Issues**: Track work with issues for complex tasks
3. **Commit Often**: Commit `.swissarmyhammer/issues/` and `.swissarmyhammer/memos/`
4. **Clean Up**: Complete issues when done

### Performance Tips

1. **Targeted Indexing**: Index only the files you need to search
2. **Specific Patterns**: Use precise glob patterns
3. **Limit Results**: Use reasonable limits for search results
4. **Incremental Indexing**: Let the tool handle incremental updates

## Troubleshooting

### Server Won't Start

```bash
# Check working directory
pwd

# Verify installation
sah --version

# Try with debug logging
RUST_LOG=debug sah serve
```

### No Search Results

```bash
# Verify index exists
ls -la .swissarmyhammer/search.db

# Force re-index
# Then in Claude: "Re-index all files with force: true"
```

### File Operations Fail

```bash
# Check permissions
ls -la

# Verify paths are absolute
# Always use full paths with file tools
```

## Next Steps

- [Configuration](./configuration.md): Customize SwissArmyHammer for your workflow
- [Features](./features.md): Explore all available tools in depth
- [Architecture](./architecture.md): Understand how SwissArmyHammer works
- [Troubleshooting](./troubleshooting.md): Solve common problems

## Example Session

Here's a complete example session:

**You:** "I want to add error handling to my project. Help me get started."

**Claude using SwissArmyHammer:**

1. Creates an issue: `issue_create` with name "add-error-handling"
2. Searches for existing patterns: `search_query` for "error handling"
3. Generates outline: `outline_generate` to understand structure
4. Searches for TODO comments: `files_grep` for "TODO.*error"
5. Reads relevant files: `files_read` for files with errors
6. Makes changes: `files_edit` to add error handling
7. Checks quality: `rules_check` to verify changes
8. Tracks changes: `git_changes` to see modifications
9. Updates issue: `issue_update` with progress
10. Completes issue: `issue_mark_complete` when done

This demonstrates the power of combining tools to accomplish complex tasks efficiently.
