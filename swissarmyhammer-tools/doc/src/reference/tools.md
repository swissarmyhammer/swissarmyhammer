# Tool Catalog

SwissArmyHammer Tools provides a comprehensive suite of MCP tools organized by category. This reference documents all available tools, their parameters, and usage examples.

## Overview

This comprehensive reference documents all 40+ MCP tools provided by SwissArmyHammer Tools. Each tool entry includes parameter specifications, return value descriptions, usage examples, and important notes.

## Quick Reference

Common tasks and their corresponding tools:

| Task | Tool | Quick Example |
|------|------|---------------|
| Read file contents | `files_read` | `{"path": "/absolute/path/to/file"}` |
| Find files by pattern | `files_glob` | `{"pattern": "**/*.rs"}` |
| Search file contents | `files_grep` | `{"pattern": "TODO", "output_mode": "content"}` |
| Modify file precisely | `files_edit` | `{"file_path": "...", "old_string": "...", "new_string": "..."}` |
| Create or overwrite file | `files_write` | `{"file_path": "...", "content": "..."}` |
| Index code for search | `search_index` | `{"patterns": ["src/**/*.rs"]}` |
| Search code semantically | `search_query` | `{"query": "authentication logic"}` |
| Create work item | `issue_create` | `{"name": "feature_name", "content": "# Title\n..."}` |
| List all issues | `issue_list` | `{"show_completed": false}` |
| View specific issue | `issue_show` | `{"name": "current"}` |
| Complete an issue | `issue_mark_complete` | `{"name": "issue_name"}` |
| Create a memo | `memo_create` | `{"title": "...", "content": "..."}` |
| Get all memos | `memo_get_all_context` | `{}` |
| Add todo task | `todo_create` | `{"task": "...", "context": "..."}` |
| Show next todo | `todo_show` | `{"item": "next"}` |
| List changed files | `git_changes` | `{"branch": "current"}` |
| Run shell command | `shell_execute` | `{"command": "cargo test"}` |
| Generate code outline | `outline_generate` | `{"patterns": ["src/**/*.rs"]}` |
| Check code quality | `rules_check` | `{"file_paths": ["src/**/*.rs"]}` |
| Fetch web page | `web_fetch` | `{"url": "https://..."}` |
| Search the web | `web_search` | `{"query": "rust async"}` |
| Execute workflow | `flow` | `{"flow_name": "deploy"}` |

## Tool Categories

- [File Operations](#file-operations) (5 tools)
- [Semantic Search](#semantic-search) (2 tools)
- [Issue Management](#issue-management) (6 tools)
- [Memo System](#memo-system) (4 tools)
- [Todo Tracking](#todo-tracking) (3 tools)
- [Git Operations](#git-operations) (1 tool)
- [Shell Execution](#shell-execution) (1 tool)
- [Code Analysis](#code-analysis) (1 tool)
- [Rules Checking](#rules-checking) (1 tool)
- [Web Operations](#web-operations) (2 tools)
- [Workflow Execution](#workflow-execution) (1 tool)
- [Flow Control](#flow-control) (1 tool)

## File Operations

### files_read

Read file contents with optional partial reading.

**Parameters**:
- `path` (string, required): **Absolute path** to the file (relative paths are not supported)
- `offset` (number, optional): Starting line number (1-based)
- `limit` (number, optional): Maximum number of lines to read

**Returns**: Object containing file content (text or base64 for binary), content type, encoding, and line counts (total, returned, offset)

**Example**:
```json
{
  "path": "/workspace/src/main.rs"
}
```

**Example with partial read**:
```json
{
  "path": "/workspace/logs/app.log",
  "offset": 1000,
  "limit": 100
}
```

### files_write

Write content to a file, creating or overwriting.

**Parameters**:
- `file_path` (string, required): **Absolute path** for the file (relative paths are not supported)
- `content` (string, required): Complete file content

**Returns**: Object containing file path and file size in bytes

**Example**:
```json
{
  "file_path": "/workspace/src/config.rs",
  "content": "// Configuration\npub const VERSION: &str = \"1.0\";"
}
```

### files_edit

Perform precise string replacement in a file.

**Parameters**:
- `file_path` (string, required): **Absolute path** to the file (relative paths are not supported)
- `old_string` (string, required): Exact text to replace
- `new_string` (string, required): Replacement text
- `replace_all` (boolean, optional): Replace all occurrences (default: false)

**Returns**: Object containing file path, number of replacements made, bytes written, encoding, and line ending format

**Example**:
```json
{
  "file_path": "/workspace/src/main.rs",
  "old_string": "const DEBUG: bool = true;",
  "new_string": "const DEBUG: bool = false;"
}
```

### files_glob

Find files matching glob patterns.

**Parameters**:
- `pattern` (string, required): Glob pattern (e.g., `**/*.rs`)
- `path` (string, optional): **Absolute path** to directory to search (default: current directory; relative paths are not supported)
- `case_sensitive` (boolean, optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (boolean, optional): Honor .gitignore (default: true)

**Returns**: Object containing file count and array of matching absolute file paths sorted by modification time

**Progress Notifications**: Reports pattern matching progress across large directory trees with file counts and completion percentage

**Example**:
```json
{
  "pattern": "src/**/*_test.rs"
}
```

### files_grep

Search file contents using regex patterns.

**Parameters**:
- `pattern` (string, required): Regular expression pattern
- `path` (string, optional): **Absolute path** to file or directory to search (relative paths are not supported)
- `glob` (string, optional): Glob pattern to filter files
- `type` (string, optional): File type filter (e.g., `js`, `py`, `rust`)
- `case_insensitive` (boolean, optional): Case-insensitive search
- `context_lines` (number, optional): Lines of context around matches
- `output_mode` (string, optional): `content`, `files_with_matches`, or `count`

**Returns**: Varies by output_mode: `content` returns matches with file paths, line numbers, and matching lines; `files_with_matches` returns array of file paths; `count` returns match counts per file

**Progress Notifications**: Reports content search progress with file and match counts as files are searched

**Example**:
```json
{
  "pattern": "fn\\s+\\w+\\s*\\(",
  "type": "rust",
  "output_mode": "content"
}
```

## Semantic Search

### search_index

Index files for semantic code search.

**Parameters**:
- `patterns` (array of strings, required): Glob patterns to match files
- `force` (boolean, optional): Force re-indexing (default: false)

**Returns**: Object containing indexed file count, skipped files count, total chunks created, and execution time in seconds

**Progress Notifications**: Sends real-time updates during indexing with file counts and percentage complete. Progress is reported in batches as files are processed.

**Example**:
```json
{
  "patterns": ["src/**/*.rs", "tests/**/*.rs"]
}
```

### search_query

Perform semantic search across indexed files.

**Parameters**:
- `query` (string, required): Search query
- `limit` (number, optional): Maximum results (default: 10)

**Returns**: Array of search results, each containing file path, chunk text, line numbers (start, end), similarity score, language, and excerpt context

**Example**:
```json
{
  "query": "authentication logic",
  "limit": 20
}
```

## Issue Management

### issue_create

Create a new issue as a markdown file.

**Parameters**:
- `content` (string, required): Markdown content
- `name` (string, optional): Issue name (auto-generated if omitted)

**Returns**: Object containing the created issue name and confirmation message

**Example**:
```json
{
  "name": "feature_001_auth",
  "content": "# User Authentication\\n\\nImplement secure authentication..."
}
```

### issue_list

List all issues with optional filtering.

**Parameters**:
- `show_completed` (boolean, optional): Include completed issues (default: false)
- `show_active` (boolean, optional): Include active issues (default: true)
- `format` (string, optional): Output format: `table`, `json`, or `markdown` (default: table)

**Returns**: Formatted list of issues, each containing issue name, status, creation date, and file path

**Example**:
```json
{
  "show_completed": true,
  "format": "markdown"
}
```

### issue_show

Display details of a specific issue.

**Parameters**:
- `name` (string, required): Issue name, "current", or "next"
- `raw` (boolean, optional): Show raw content only (default: false)

**Returns**: Object containing issue name, status, creation date, file path, and markdown content (when raw=true, returns only markdown content)

**Example**:
```json
{
  "name": "current"
}
```

### issue_update

Update an existing issue's content.

**Parameters**:
- `name` (string, required): Issue name to update
- `content` (string, required): New markdown content
- `append` (boolean, optional): Append instead of replacing (default: false)

**Returns**: Confirmation

**Example**:
```json
{
  "name": "feature_001_auth",
  "content": "# Updated Content\\n\\nNew requirements...",
  "append": false
}
```

### issue_mark_complete

Mark an issue as complete.

**Parameters**:
- `name` (string, required): Issue name or "current"

**Returns**: Confirmation message that the issue has been marked complete and moved to the complete directory

**Example**:
```json
{
  "name": "feature_001_auth"
}
```

### issue_all_complete

Check if all issues are completed.

**Parameters**: None

**Returns**: Boolean indicating completion status and counts

**Example**:
```json
{}
```

## Memo System

### memo_create

Create a new memo with title and content.

**Parameters**:
- `title` (string, required): Memo title
- `content` (string, required): Markdown content

**Returns**: Object containing the created memo with ULID identifier, title, content, and creation timestamp

**Example**:
```json
{
  "title": "Architecture Decisions",
  "content": "# System Architecture\\n\\nWe decided to use microservices..."
}
```

### memo_list

List all available memos.

**Parameters**: None

**Returns**: List of memos with titles, IDs, and previews

**Example**:
```json
{}
```

### memo_get

Retrieve a specific memo by title.

**Parameters**:
- `title` (string, required): Memo title

**Returns**: Memo content with metadata

**Example**:
```json
{
  "title": "Architecture Decisions"
}
```

### memo_get_all_context

Get all memo content for AI context.

**Parameters**: None

**Returns**: All memos concatenated, sorted by recency

**Example**:
```json
{}
```

## Todo Tracking

### todo_create

Add an item to the todo list.

**Parameters**:
- `task` (string, required): Brief task description
- `context` (string, optional): Additional context or notes

**Returns**: Confirmation with ULID

**Example**:
```json
{
  "task": "Implement file validation",
  "context": "Check for valid file extensions and sizes"
}
```

### todo_show

Retrieve a specific todo item or the next incomplete item.

**Parameters**:
- `item` (string, required): ULID or "next"

**Returns**: Todo item details

**Example**:
```json
{
  "item": "next"
}
```

### todo_mark_complete

Mark a todo item as completed.

**Parameters**:
- `id` (string, required): ULID of the todo item

**Returns**: Confirmation

**Example**:
```json
{
  "id": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

## Git Operations

### git_changes

List files changed on a branch relative to parent.

**Parameters**:
- `branch` (string, required): Branch name to analyze

**Returns**: Branch name, parent, and list of changed files

**Example**:
```json
{
  "branch": "issue/feature-123"
}
```

## Shell Execution

### shell_execute

Execute shell commands with proper output handling.

**Parameters**:
- `command` (string, required): Shell command to execute
- `working_directory` (string, optional): **Absolute path** for working directory (relative paths are not supported)
- `environment` (string, optional): JSON string of environment variables

**Returns**: Object containing command output (stdout/stderr combined), exit code, execution time in milliseconds, and working directory path

**Progress Notifications**: Streams command output in real-time as lines are produced by the executing command, enabling live feedback for long-running operations.

**Example**:
```json
{
  "command": "cargo test",
  "working_directory": "/workspace"
}
```

## Code Analysis

### outline_generate

Generate structured code outlines using tree-sitter.

**Parameters**:
- `patterns` (array of strings, required): Glob patterns for files
- `output_format` (string, optional): `yaml` or `json` (default: yaml)

**Returns**: Hierarchical outline containing file paths, symbols (classes, functions, methods, etc.) with line numbers, signatures, and documentation

**Progress Notifications**: Reports parsing progress across multiple source files with file counts and percentage complete

**Example**:
```json
{
  "patterns": ["src/**/*.rs"],
  "output_format": "yaml"
}
```

## Rules Checking

### rules_check

Check source code against quality rules.

**Parameters**:
- `rule_names` (array of strings, optional): Specific rules to check
- `file_paths` (array of strings, optional): Files or patterns to check
- `category` (string, optional): Category filter
- `severity` (string, optional): Severity filter (error, warning, info, hint)

**Returns**: List of violations

**Progress Notifications**: Reports rule checking progress with file counts and completion status as files are analyzed

**Example**:
```json
{
  "file_paths": ["src/**/*.rs"],
  "severity": "error"
}
```

## Web Operations

### web_fetch

Fetch web content and convert to markdown.

**Parameters**:
- `url` (string, required): URL to fetch (HTTP/HTTPS only)
- `timeout` (number, optional): Request timeout in seconds (default: 30, max: 120)
- `follow_redirects` (boolean, optional): Follow redirects (default: true)
- `max_content_length` (number, optional): Max content size in bytes (default: 1MB, max: 10MB)
- `user_agent` (string, optional): Custom User-Agent header

**Returns**: Converted markdown content

**Progress Notifications**: Tracks HTTP request and HTML-to-markdown conversion progress

**Example**:
```json
{
  "url": "https://docs.example.com/api",
  "timeout": 60
}
```

### web_search

Search the web using DuckDuckGo.

**Parameters**:
- `query` (string, required): Search query (1-500 characters)
- `category` (string, optional): Search category (general, images, videos, news, it, etc.)
- `language` (string, optional): Language code (default: en)
- `results_count` (number, optional): Number of results (default: 10, max: 50)
- `fetch_content` (boolean, optional): Fetch and convert page content (default: true)
- `safe_search` (string, optional): Safe search level (Off, Moderate, Strict)
- `time_range` (string, optional): Time filter (day, week, month, year)

**Returns**: Search results with optional content

**Progress Notifications**: Reports search execution progress and content fetching from result URLs

**Example**:
```json
{
  "query": "rust async programming",
  "category": "it",
  "results_count": 15
}
```

## Workflow Execution

### flow

Execute or list workflows dynamically via MCP.

**Parameters**:
- `flow_name` (string, required): Name of the workflow to execute, or "list" to show all workflows
- `parameters` (object, optional): Workflow-specific parameters as key-value pairs (ignored when flow_name='list')
- `format` (string, optional): Output format when flow_name='list' (json, yaml, or table)
- `verbose` (boolean, optional): Include detailed parameter information when flow_name='list'
- `interactive` (boolean, optional): Enable interactive mode for prompts (workflow execution only)
- `dry_run` (boolean, optional): Show execution plan without running (workflow execution only)
- `quiet` (boolean, optional): Suppress progress output (workflow execution only)

**Returns**: Workflow execution results or list of available workflows

**Progress Notifications**: Reports comprehensive workflow progress including:
- Flow start with initial state and parameters
- State transitions with state ID and description
- Step execution completion with next state information
- Flow completion with final status and state
- Error notifications with state and error details
See the [Features documentation](../features.md#progress-notifications) for detailed notification structure

**Example (List Workflows)**:
```json
{
  "flow_name": "list",
  "verbose": true
}
```

**Example (Execute Workflow)**:
```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  },
  "interactive": false
}
```

**Progress Notifications**: Long-running workflows send MCP progress notifications to track execution state, including flow start, state transitions, and completion or error events.

## Flow Control

### abort_create

Create an abort signal for workflow termination.

**Parameters**:
- `reason` (string, required): Reason for aborting

**Returns**: Confirmation with abort reason

**Example**:
```json
{
  "reason": "User cancelled the destructive operation"
}
```

## Tool Naming Convention

All tools follow the `{category}_{action}` naming pattern:

- **Category**: Logical grouping (memo, issue, files, etc.)
- **Action**: Operation to perform (create, read, update, delete, etc.)

Examples:
- `memo_create`: Create a memo
- `issue_list`: List issues
- `files_read`: Read a file

## Common Parameters

### Path Parameters

All file paths must be **absolute paths**:

```json
{
  "path": "/Users/name/project/src/main.rs"  // ✓ Correct
}
```

Not relative paths:

```json
{
  "path": "./src/main.rs"  // ✗ May fail
}
```

### Boolean Parameters

Use `true` or `false` (not quoted):

```json
{
  "force": true,      // ✓ Correct
  "append": false     // ✓ Correct
}
```

### Optional Parameters

Omit optional parameters to use defaults:

```json
{
  "pattern": "**/*.rs"
  // case_sensitive omitted, defaults to false
}
```

## Error Handling

All tools return structured error responses:

```json
{
  "error": {
    "code": "InvalidParameter",
    "message": "File not found: /path/to/file.txt",
    "details": {
      "parameter": "path",
      "value": "/path/to/file.txt"
    }
  }
}
```

Common error codes:
- `InvalidParameter`: Parameter validation failed
- `NotFound`: Resource not found
- `PermissionDenied`: Insufficient permissions
- `InternalError`: Unexpected server error

## Usage in Claude Desktop

Tools are automatically available in Claude Desktop when SwissArmyHammer is configured as an MCP server.

**Natural language interface**:
```
Instead of calling tools directly, describe what you want:

"Create an issue for implementing authentication"
→ Uses issue_create tool

"Show me all Rust files in src/"
→ Uses files_glob tool

"Search for error handling patterns"
→ Uses search_query tool
```

## Related Documentation

- [Features Overview](../features.md)
- [Architecture Overview](../architecture.md)
- [Configuration Reference](./configuration.md)
