# MCP Tools Overview

SwissArmyHammer provides 28 MCP tools that integrate seamlessly with Claude Code and other MCP clients. These tools are automatically exposed via both the MCP protocol and the CLI interface.

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
- `issue_all_complete` - Check if all issues are completed

### Todo Operations
Ephemeral task tracking for development sessions:
- `todo_create` - Create new todo items
- `todo_show` - Retrieve specific todo item or next incomplete
- `todo_mark_complete` - Mark todo items as completed

### Memoranda Operations
Personal knowledge management with full-text search:
- `memo_create` - Create new memos with titles and content
- `memo_get` - Retrieve specific memo by title
- `memo_list` - List all available memos
- `memo_get_all_context` - Get all memo content for AI context

### Semantic Search
Vector-based code search with TreeSitter parsing:
- `search_index` - Index code files for semantic search
- `search_query` - Query indexed content with natural language

### Outline Operations
Generate structured code overviews:
- `outline_generate` - Create hierarchical code outlines with symbols

### Git Operations
Git repository analysis and file change tracking:
- `git_changes` - List files changed on a branch relative to parent

### Rules Operations
Automated code quality checking:
- `rules_check` - Check code against defined quality rules

### Flow Operations
Dynamic workflow execution via MCP:
- `flow` - Execute or list workflows with parameters

### Shell Integration
Safe command execution with security controls:
- `shell_execute` - Execute shell commands with timeout and validation

### Web Tools
Internet integration for data gathering:
- `web_fetch` - Fetch and convert web pages to markdown
- `web_search` - Search the web with DuckDuckGo integration

### Abort Operations
Workflow termination signaling:
- `abort_create` - Signal workflow termination with reason

## Usage Patterns

Tools can be used in three ways:

1. **Direct CLI** - `sah files read --absolute-path ./README.md`
2. **MCP Integration** - Available in Claude Code automatically
3. **Workflow Actions** - Called from workflow states

Each tool provides consistent interfaces across all usage modes with comprehensive error handling and validation.