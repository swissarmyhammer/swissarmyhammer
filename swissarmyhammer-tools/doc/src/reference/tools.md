# Tool Catalog

Complete reference of all SwissArmyHammer Tools organized by category.

## File Operations (files_*)

| Tool | Description |
|------|-------------|
| `files_read` | Read file contents with optional offset and limit |
| `files_write` | Write content to file atomically |
| `files_edit` | Perform precise string replacement |
| `files_glob` | Find files matching glob patterns |
| `files_grep` | Search file contents using ripgrep |

## Search Operations (search_*)

| Tool | Description |
|------|-------------|
| `search_index` | Index files for semantic search using tree-sitter |
| `search_query` | Query indexed code with semantic similarity |

## Issue Management (issue_*)

| Tool | Description |
|------|-------------|
| `issue_create` | Create new work item with markdown content |
| `issue_list` | List issues with filtering options |
| `issue_show` | Display details of specific issue |
| `issue_update` | Update issue content (append or replace) |
| `issue_mark_complete` | Mark issue as complete and archive |
| `issue_all_complete` | Check if all issues are completed |

## Memoranda Operations (memo_*)

| Tool | Description |
|------|-------------|
| `memo_create` | Create note with title and content |
| `memo_get` | Retrieve specific memo by title |
| `memo_list` | List all memos with previews |
| `memo_get_all_context` | Get aggregated memo content |

## Todo Operations (todo_*)

| Tool | Description |
|------|-------------|
| `todo_create` | Create ephemeral task item |
| `todo_show` | Show specific todo or next incomplete |
| `todo_mark_complete` | Mark todo as complete |

## Git Operations (git_*)

| Tool | Description |
|------|-------------|
| `git_changes` | List files changed on branch with parent detection |

## Shell Operations (shell_*)

| Tool | Description |
|------|-------------|
| `shell_execute` | Execute shell command with environment control |

## Outline Operations (outline_*)

| Tool | Description |
|------|-------------|
| `outline_generate` | Generate code outline using tree-sitter |

## Rules Operations (rules_*)

| Tool | Description |
|------|-------------|
| `rules_check` | Check code against quality rules |

## Web Operations (web_*)

| Tool | Description |
|------|-------------|
| `web_fetch` | Fetch web content and convert to markdown |
| `web_search` | Search using DuckDuckGo with content fetching |

## Flow Operations (flow)

| Tool | Description |
|------|-------------|
| `flow` | Execute workflow with AI agent coordination |

## Abort Operations (abort_*)

| Tool | Description |
|------|-------------|
| `abort_create` | Create abort signal to terminate workflows |

## Tool Parameters

### Common Parameter Types

**Path Parameters:**
- `path`, `file_path`: File system paths
- Relative to working directory or absolute
- Validated and canonicalized

**Pattern Parameters:**
- `pattern`: Glob or regex patterns
- Syntax depends on tool (glob vs regex)

**Boolean Parameters:**
- `force`, `replace_all`, `case_sensitive`
- Optional, usually default to false

**Limit Parameters:**
- `limit`, `max_results`, `offset`
- Control result set size and pagination

## Tool Response Formats

### Success Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    // Tool-specific data
  }
}
```text

### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": {
      "details": "Error description"
    }
  }
}
```text

## Tool Compatibility

### Language Support

**Search and Outline Tools:**
- Rust
- Python
- TypeScript/JavaScript
- Dart
- Additional languages via tree-sitter

**File Tools:**
- All text-based files
- Binary file detection
- Encoding detection

### Platform Support

**All Platforms:**
- macOS
- Linux
- Windows

**Platform-Specific:**
- Shell execution depends on available shell
- Path handling adapts to platform conventions

## Next Steps

- **[API Documentation](api.md)** - Rust API reference
- **[Features](../features.md)** - Detailed feature documentation
