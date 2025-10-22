# Tool Catalog

Complete reference of all SwissArmyHammer Tools organized by category.

## File Tools (5 tools)

### files_read

Read file contents from the local filesystem with partial reading support.

**Parameters:**
- `path` (required): File path (absolute or relative)
- `offset` (optional): Starting line number (1-based)
- `limit` (optional): Maximum lines to read

**Use Cases:** Reading source files, configuration files, logs

---

### files_write

Write content to files with atomic operations.

**Parameters:**
- `file_path` (required): Absolute path for the file
- `content` (required): Complete file content

**Use Cases:** Creating new files, overwriting existing files

---

### files_edit

Perform precise string replacements in files.

**Parameters:**
- `file_path` (required): Absolute path to file
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences

**Use Cases:** Updating configuration values, refactoring code

---

### files_glob

Fast file pattern matching with .gitignore support.

**Parameters:**
- `pattern` (required): Glob pattern (e.g., `**/*.rs`)
- `path` (optional): Directory to search
- `case_sensitive` (optional): Case-sensitive matching
- `respect_git_ignore` (optional): Honor .gitignore

**Use Cases:** Finding files by name pattern, listing project files

---

### files_grep

Content-based search with ripgrep.

**Parameters:**
- `pattern` (required): Regular expression pattern
- `path` (optional): File or directory
- `glob` (optional): File filter pattern
- `type` (optional): File type (js, py, rust, etc.)
- `case_insensitive` (optional): Case-insensitive search
- `context_lines` (optional): Context lines around matches
- `output_mode` (optional): content, files_with_matches, count

**Use Cases:** Searching code, finding TODOs, locating definitions

---

## Search Tools (2 tools)

### search_index

Index files for semantic search using vector embeddings.

**Parameters:**
- `patterns` (required): Array of glob patterns
- `force` (optional): Force re-indexing

**Use Cases:** Initial codebase indexing, updating search index

---

### search_query

Perform semantic search across indexed files.

**Parameters:**
- `query` (required): Search query string
- `limit` (optional): Number of results (default: 10)

**Use Cases:** Finding relevant code, exploring codebase, locating features

---

## Issue Tools (6 tools)

### issue_create

Create a new issue stored as a markdown file.

**Parameters:**
- `content` (required): Markdown content
- `name` (optional): Issue name (auto-generated if omitted)

**Use Cases:** Creating work items, tracking tasks, documenting bugs

---

### issue_list

List all available issues with status and metadata.

**Parameters:**
- `show_completed` (optional): Include completed issues
- `show_active` (optional): Include active issues
- `format` (optional): table, json, or markdown

**Use Cases:** Viewing all work items, tracking progress

---

### issue_show

Display details of a specific issue.

**Parameters:**
- `name` (required): Issue name (or "next" for next pending)
- `raw` (optional): Show raw content only

**Use Cases:** Viewing issue details, workflow automation

---

### issue_update

Update the content of an existing issue.

**Parameters:**
- `name` (required): Issue name
- `content` (required): New content
- `append` (optional): Append to existing content

**Use Cases:** Updating progress, adding notes, modifying requirements

---

### issue_mark_complete

Mark an issue as complete.

**Parameters:**
- `name` (required): Issue name

**Use Cases:** Completing work items, archiving issues

---

### issue_all_complete

Check if all issues are completed.

**Parameters:** None

**Use Cases:** Workflow completion checking, project status

---

## Memo Tools (4 tools)

### memo_create

Create a new memo with title and content.

**Parameters:**
- `title` (required): Memo title
- `content` (required): Markdown content

**Use Cases:** Note-taking, documenting decisions, saving research

---

### memo_list

List all available memos.

**Parameters:** None

**Use Cases:** Browsing notes, finding information

---

### memo_get

Retrieve a memo by title.

**Parameters:**
- `title` (required): Memo title

**Use Cases:** Reading specific notes, referencing information

---

### memo_get_all_context

Get all memo content formatted for AI context.

**Parameters:** None

**Use Cases:** Loading project context, AI prompting

---

## Todo Tools (3 tools)

### todo_create

Add a new item to a todo list.

**Parameters:**
- `task` (required): Task description
- `context` (optional): Additional context or notes

**Use Cases:** Ephemeral task tracking, development sessions

---

### todo_show

Retrieve a specific todo item or the next incomplete item.

**Parameters:**
- `item` (required): ULID or "next"

**Use Cases:** Viewing todos, workflow automation

---

### todo_mark_complete

Mark a todo item as completed.

**Parameters:**
- `id` (required): ULID of todo item

**Use Cases:** Completing tasks, cleaning up todo list

---

## Git Tools (1 tool)

### git_changes

List files that have changed on a branch.

**Parameters:**
- `branch` (required): Branch name to analyze

**Use Cases:** Reviewing changes, tracking modifications, PR preparation

---

## Shell Tools (1 tool)

### shell_execute

Execute shell commands with environment control.

**Parameters:**
- `command` (required): Shell command to execute
- `working_directory` (optional): Working directory
- `environment` (optional): Additional environment variables (JSON)

**Use Cases:** Running builds, executing tests, system operations

---

## Outline Tools (1 tool)

### outline_generate

Generate structured code overviews using Tree-sitter.

**Parameters:**
- `patterns` (required): Array of glob patterns
- `output_format` (optional): yaml or json

**Use Cases:** Code structure analysis, documentation generation, codebase understanding

---

## Rules Tools (1 tool)

### rules_check

Check source code against SwissArmyHammer rules.

**Parameters:**
- `rule_names` (optional): Specific rules to check
- `file_paths` (optional): Files or patterns to check
- `category` (optional): Category filter
- `severity` (optional): error, warning, info, hint

**Use Cases:** Code quality checking, standards compliance, lint checking

---

## Web Tools (2 tools)

### web_fetch

Fetch web content and convert HTML to markdown.

**Parameters:**
- `url` (required): URL to fetch
- `timeout` (optional): Request timeout in seconds
- `follow_redirects` (optional): Follow redirects
- `max_content_length` (optional): Maximum content length
- `user_agent` (optional): Custom User-Agent

**Use Cases:** Fetching documentation, retrieving web content, research

---

### web_search

Perform web searches using DuckDuckGo.

**Parameters:**
- `query` (required): Search query (1-500 characters)
- `category` (optional): general, images, videos, news, it, etc.
- `language` (optional): Language code (default: en)
- `results_count` (optional): Number of results (max: 50)
- `fetch_content` (optional): Fetch and convert pages
- `safe_search` (optional): 0 (off), 1 (moderate), 2 (strict)
- `time_range` (optional): "", day, week, month, year

**Use Cases:** Web research, finding documentation, gathering information

---

## Flow Tools (1 tool)

### flow

Execute or list workflows dynamically.

**Parameters:**
- `flow_name` (required): Workflow name or "list"
- `parameters` (optional): Workflow-specific parameters
- `format` (optional): json, yaml, or table (for list)
- `verbose` (optional): Include detailed information
- `interactive` (optional): Enable interactive mode
- `dry_run` (optional): Show execution plan
- `quiet` (optional): Suppress progress output

**Use Cases:** Workflow automation, orchestration, complex task execution

---

## Abort Tools (1 tool)

### abort_create

Create an abort file to signal workflow termination.

**Parameters:**
- `reason` (required): Abort reason/message

**Use Cases:** Workflow termination, error handling, safety stops

---

## Tool Statistics

- **Total Tools**: 28
- **Categories**: 11
- **Most Tools**: File operations (5 tools)
- **Supported Languages**: Rust, Python, TypeScript, JavaScript, Dart

## Tool Selection Guide

### For File Operations
- Reading: `files_read`
- Writing: `files_write`
- Editing: `files_edit`
- Finding: `files_glob`
- Searching: `files_grep`

### For Code Understanding
- Semantic search: `search_index` + `search_query`
- Structure: `outline_generate`
- Content search: `files_grep`

### For Project Management
- Work tracking: `issue_*` tools
- Notes: `memo_*` tools
- Tasks: `todo_*` tools

### For Development
- Changes: `git_changes`
- Commands: `shell_execute`
- Quality: `rules_check`

### For Research
- Web search: `web_search`
- Content fetch: `web_fetch`
- Save notes: `memo_create`

## Next Steps

- [Features](./features.md): Detailed feature documentation
- [Quick Start](./quick-start.md): Try the tools
- [Architecture](./architecture.md): Understand the system
