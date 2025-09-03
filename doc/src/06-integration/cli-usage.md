# CLI Usage

SwissArmyHammer provides a comprehensive CLI interface that exposes all MCP tools as command-line utilities. The CLI dynamically generates commands from MCP tool definitions, ensuring perfect consistency between MCP and CLI interfaces.

## Command Structure

```bash
sah <category> <tool> [options]
```

Where:
- `<category>` - Tool category (files, issue, memo, search, etc.)
- `<tool>` - Specific tool within category
- `[options]` - Tool-specific parameters

## Available Categories

### File Operations
```bash
# Read files
sah file read --absolute-path ./src/main.rs

# Write files
sah file write --file-path ./output.txt --content "Hello World"

# Edit files
sah file edit --file-path ./config.toml --old-string "debug = false" --new-string "debug = true"

# Find files
sah file glob --pattern "**/*.rs"

# Search content
sah file grep --pattern "TODO" --output-mode content
```

### Issue Management
```bash
# Create issue
sah issue create --content "# Bug Fix\n\nDetails..."

# Start work
sah issue work --name "FEATURE_000123_user-auth"

# Complete issue
sah issue complete --name "FEATURE_000123_user-auth"

# List issues
sah issue list

# Show issue details
sah issue show --name "FEATURE_000123_user-auth"

# Check completion status
sah issue status
```

### Memo System
```bash
# Create memo
sah memo create --title "Meeting Notes" --content "# Notes\n\nDiscussion points..."

# List memos
sah memo list

# Search memos
sah memo search --query "project roadmap"

# Get memo context for AI
sah memo context
```

### Semantic Search
```bash
# Index files
sah search index --patterns "**/*.rs" "**/*.ts"

# Query indexed content
sah search query --query "error handling patterns"
```

### Web Tools
```bash
# Fetch web content
sah web-search search --query "rust async programming" --results-count 10
```

### Shell Execution
```bash
# Execute shell commands
sah shell execute --command "cargo test" --timeout 600
```

## MCP vs CLI Mapping

The CLI provides a direct mapping from MCP tools:

| MCP Tool Name | CLI Command |
|---------------|-------------|
| `files_read` | `sah file read` |
| `files_write` | `sah file write` |
| `files_edit` | `sah file edit` |
| `files_glob` | `sah file glob` |
| `files_grep` | `sah file grep` |
| `issue_create` | `sah issue create` |
| `issue_work` | `sah issue work` |
| `issue_complete` | `sah issue complete` |
| `memo_create` | `sah memo create` |
| `memo_search` | `sah memo search` |
| `search_index` | `sah search index` |
| `search_query` | `sah search query` |

## Global Options

```bash
# Verbose output
sah -v files read --absolute-path ./main.rs

# Debug output
sah -d issue create --content "Debug issue"

# Quiet mode (errors only)
sah -q files glob --pattern "**/*.rs"

# Validate all tool schemas
sah --validate-tools
```

## Configuration

The CLI respects the same configuration as the MCP server:
- Uses current working directory as workspace root
- Respects `.gitignore` patterns
- Follows security boundaries for file operations

## Integration with Claude Code

When using Claude Code, MCP tools are automatically available:

```
User: Use files_read to examine the main.rs file
Claude: [Uses MCP tool directly]

User: List all TypeScript files
Claude: [Uses files_glob with pattern "**/*.ts"]
```

The CLI provides the same functionality for direct command-line usage:

```bash
# Equivalent CLI commands
sah file read --absolute-path ./src/main.rs
sah file glob --pattern "**/*.ts"
```

## Error Handling

The CLI provides detailed error messages and follows standard exit codes:
- `0` - Success
- `1` - General error
- `2` - Invalid arguments
- `130` - Interrupted by user (Ctrl+C)

Error messages include context and suggestions for resolution:

```bash
$ sah files read --absolute-path /nonexistent
Error: File not found: /nonexistent
Suggestion: Check that the file path is correct and the file exists
```

## Help System

Every command and subcommand provides built-in help:

```bash
# Main help
sah --help

# Category help
sah file --help

# Tool-specific help
sah file read --help
```

Help includes:
- Command description
- Required and optional parameters
- Usage examples
- Related commands