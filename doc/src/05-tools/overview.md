# MCP Tools Overview

SwissArmyHammer provides 25+ MCP tools that integrate seamlessly with Claude Code and other MCP clients. These tools are automatically exposed via both the MCP protocol and the CLI interface.

## Tool Categories

### File Operations
Core file system operations for reading, writing, and manipulating files:
- `files_read` - Read file contents with partial reading support
- `files_write` - Write new files or overwrite existing ones
- `files_edit` - Precise string replacements in existing files
- `files_glob` - Pattern-based file discovery with gitignore support
- `files_grep` - Content-based search with regex support

### Issue Management
Git-integrated project management using markdown files:
- `issue_create` - Create new issues as markdown files
- `issue_mark_complete` - Mark issues as complete and archive
- `issue_list` - List all issues with filtering
- `issue_show` - Display issue details
- `issue_update` - Update issue content

### Memo System
Personal knowledge management with full-text search:
- `memo_create` - Create new memos with titles and content
- `memo_search` - Search memos by content
- `memo_list` - List all available memos
- `memo_get` - Retrieve specific memo by ID

### Semantic Search
Vector-based code search with TreeSitter parsing:
- `search_index` - Index code files for semantic search
- `search_query` - Query indexed content with natural language

### Web Tools
Internet integration for data gathering:
- `web_fetch` - Fetch and convert web pages to markdown
- `web_search` - Search the web with DuckDuckGo integration

### Shell Integration
Safe command execution with security controls:
- `shell_execute` - Execute shell commands with timeout and validation

### Utility Tools
Supporting functionality for workflows:
- `todo_create` - Ephemeral task tracking
- `abort_create` - Signal workflow termination
- `outline_generate` - Generate code structure overviews

## Usage Patterns

Tools can be used in three ways:

1. **Direct CLI** - `sah files read --absolute-path ./README.md`
2. **MCP Integration** - Available in Claude Code automatically
3. **Workflow Actions** - Called from workflow states

Each tool provides consistent interfaces across all usage modes with comprehensive error handling and validation.