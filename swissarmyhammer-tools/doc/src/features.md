# Features

SwissArmyHammer Tools provides a comprehensive suite of capabilities for AI-assisted software development. Each feature area is designed to work seamlessly with AI assistants like Claude to enhance development workflows.

## Quick Reference

| Feature | What It Does | Key Tools |
|---------|--------------|-----------|
| [File Tools](#file-tools) | Read, write, edit, search files | `files_read`, `files_write`, `files_edit`, `files_glob`, `files_grep` |
| [Semantic Search](#semantic-search) | AI-powered code search | `search_index`, `search_query` |
| [Issue Management](#issue-management) | Track work items | `issue_create`, `issue_list`, `issue_mark_complete` |
| [Memo System](#memo-system) | Knowledge management | `memo_create`, `memo_get`, `memo_list` |
| [Todo Tracking](#todo-tracking) | Ephemeral task lists | `todo_create`, `todo_show`, `todo_mark_complete` |
| [Git Integration](#git-integration) | Track changes | `git_changes` |
| [Shell Execution](#shell-execution) | Run commands | `shell_execute` |
| [Code Outline](#code-outline) | Analyze structure | `outline_generate` |
| [Rules Engine](#rules-engine) | Check quality | `rules_check` |
| [Web Tools](#web-tools) | Fetch content, search | `web_fetch`, `web_search` |
| [Workflow Execution](#workflow-execution) | Automate processes | `flow` |
| [Progress Notifications](#progress-notifications) | Real-time updates | Automatic for long operations |

## Overview

SwissArmyHammer Tools exposes functionality through MCP tools organized into logical categories:

- **[File Tools](#file-tools)**: Read, write, edit, and search files with security validation
- **[Semantic Search](#semantic-search)**: Vector-based code search using tree-sitter and embeddings
- **[Issue Management](#issue-management)**: Track work items through their complete lifecycle
- **[Memo System](#memo-system)**: Knowledge management and note-taking with ULID identifiers
- **[Todo Tracking](#todo-tracking)**: Ephemeral task tracking for development sessions
- **[Git Integration](#git-integration)**: Track changes with branch detection and parent tracking
- **[Shell Execution](#shell-execution)**: Execute commands with environment and output control
- **[Code Outline](#code-outline)**: Generate structured code overviews using tree-sitter
- **[Rules Engine](#rules-engine)**: Check code against quality standards and best practices
- **[Web Tools](#web-tools)**: Fetch web content and search with DuckDuckGo
- **[Workflow Execution](#workflow-execution)**: Execute complex workflows with AI coordination
- **[Progress Notifications](#progress-notifications)**: Real-time progress updates during long operations
- **[Abort Mechanism](#abort-mechanism)**: Signal workflow termination gracefully

## File Tools

File operations with security validation and atomic operations.

**Tools**: `files_read`, `files_write`, `files_edit`, `files_glob`, `files_grep`

### Key Capabilities

- Read files with partial reading support (offset and limit for large files)
- Write files atomically with permissions preservation
- Perform precise string replacements with exact matching validation
- Pattern matching with glob support and .gitignore awareness
- Content search with regex using ripgrep for performance
- Binary file detection with base64 encoding support
- Line ending normalization across platforms
- Character encoding detection and handling

### Example Usage

```
Read a configuration file:
"Show me the contents of sah.yaml"

Find all test files:
"Find all files matching **/*_test.rs"

Replace a function call:
"Replace all calls to old_function with new_function"
```

## Semantic Search

Vector-based code search using tree-sitter parsing and embeddings.

**Tools**: `search_index`, `search_query`

### Key Capabilities

- Index codebases with intelligent chunking
- Search by semantic meaning, not just keywords
- Support for Rust, Python, TypeScript, JavaScript, Dart
- Fast SQLite-backed vector storage
- Incremental indexing (only changed files)

### Example Usage

```
Index your codebase:
"Index all Rust files for semantic search"

Search for functionality:
"Search for authentication logic"
"Find error handling patterns"
```

## Issue Management

Track work items as markdown files with complete lifecycle support.

**Tools**: `issue_create`, `issue_list`, `issue_show`, `issue_update`, `issue_mark_complete`, `issue_all_complete`

### Key Capabilities

- Create issues as markdown files
- List active and completed issues
- Update issue content
- Mark issues complete (moves to completed folder)
- Check if all issues are complete
- Git-friendly storage in `.swissarmyhammer/issues`

### Example Usage

```
Create an issue:
"Create an issue for implementing user authentication"

List issues:
"Show me all active issues"

Complete an issue:
"Mark the authentication issue as complete"
```

## Memo System

Knowledge management and note-taking with ULID-based organization.

**Tools**: `memo_create`, `memo_list`, `memo_get`, `memo_get_all_context`

### Key Capabilities

- Create memos with title and markdown content
- List all memos with previews
- Retrieve specific memos by title
- Get all memo content for AI context
- ULID-based unique identifiers

### Example Usage

```
Create a memo:
"Create a memo about the authentication flow"

List memos:
"Show me all memos"

Get memo content:
"Get the memo about authentication"
```

## Todo Tracking

Ephemeral task tracking for development sessions.

**Tools**: `todo_create`, `todo_show`, `todo_mark_complete`

### Key Capabilities

- Create todo items with task and context
- Show next incomplete item
- Mark items complete
- Automatic cleanup when all tasks done
- Stored in `.swissarmyhammer/todo.yaml`

### Example Usage

```
Create a todo:
"Add a todo to implement file validation"

Show next task:
"What's the next todo item?"

Complete a task:
"Mark the current todo as complete"
```

## Git Integration

Track file changes and integrate with git workflows.

**Tools**: `git_changes`

### Key Capabilities

- List changed files on a branch
- Automatic parent branch detection
- Include uncommitted changes
- Support for issue/* branch workflows

### Example Usage

```
Show changes:
"What files have changed on this branch?"

Track work:
"Show me all files modified for this issue"
```

## Shell Execution

Execute shell commands with proper output handling.

**Tools**: `shell_execute`

### Key Capabilities

- Execute any shell command
- Custom working directory
- Environment variable support
- Capture stdout and stderr
- Execution time tracking

### Example Usage

```
Run tests:
"Run cargo nextest run"

Build the project:
"Execute cargo build --release"
```

## Code Outline

Generate structured code overviews using tree-sitter parsing.

**Tools**: `outline_generate`

### Key Capabilities

- Extract symbols (classes, functions, methods, etc.)
- Include line numbers and signatures
- Capture documentation comments
- Output as YAML or JSON
- Support for multiple languages

### Example Usage

```
Generate outline:
"Create an outline of all Rust files"

Analyze structure:
"Show me the structure of src/main.rs"
```

## Rules Engine

Check code against quality standards and best practices.

**Tools**: `rules_check`

### Key Capabilities

- Check files against defined rules
- Filter by severity (error, warning, info, hint)
- Category-based filtering
- Detailed violation reports
- Configurable rule sets

### Example Usage

```
Check code quality:
"Check all Rust files for rule violations"

Check specific rules:
"Check for unwrap usage in src/**/*.rs"
```

## Web Tools

Fetch and search web content with markdown conversion.

**Tools**: `web_fetch`, `web_search`

### Key Capabilities

- Fetch web pages and convert to markdown
- Search using DuckDuckGo
- Category-based search (general, news, IT, etc.)
- Content extraction from search results
- Safe search filtering

### Example Usage

```
Fetch documentation:
"Fetch the content from https://docs.example.com/api"

Search for solutions:
"Search for rust async programming patterns"
```

## Workflow Execution

Execute complex development workflows with AI agent coordination.

**Tools**: `flow`

### Key Capabilities

- Define workflows as YAML specifications
- Execute steps with agent coordination
- Track workflow state
- Handle errors and retries
- Support for complex multi-step processes

### Example Usage

```
Execute a workflow:
"Run the deployment workflow"

Check workflow status:
"Show me the status of the current workflow"
```

## Progress Notifications

Tools send real-time progress updates during long-running operations via MCP notifications.

**Features**:

- Streaming progress updates for shell commands
- File indexing progress with percentage complete
- Web search and fetch progress tracking
- Workflow execution state transitions
- No LLM tool calls required - server-sent automatically

### Key Capabilities

- Channel-based async notification delivery
- ULID-based progress tokens for tracking operations
- Progress percentages (0-100) or indeterminate progress
- Custom metadata support for tool-specific information
- Non-blocking operation - tools continue while sending updates

### How It Works

Tools that support progress notifications send updates through [MCP's progress notification protocol](./architecture/mcp-server.md) whenever they perform long-running operations. These notifications are triggered automatically by the tool implementation when significant progress milestones are reached (such as completing processing of a batch of files, finishing a workflow state, or receiving streaming output from a command).

Clients subscribing to MCP notifications receive these updates in real-time through the notification channel without needing to poll or make additional tool calls. The AI assistant can display this progress information to users, providing:

- Live feedback during long operations
- Better user experience with visibility into progress
- Ability to track multiple concurrent operations
- Detailed progress information with custom metadata

### Example Progress Flow

Generic progress flow showing percentage completion:
```
1. Tool starts: "Starting: file indexing" (0%)
2. Tool progressing: "Indexed 500/1000 files" (50%)
3. Tool completing: "Completed: file indexing" (100%)
```

Concrete example - Indexing 1000 Rust files:
```
1. "Starting file indexing" (0%)
2. "Indexed 250 files" (25%)
3. "Indexed 500 files" (50%)
4. "Indexed 750 files" (75%)
5. "Completed: 1000 files indexed" (100%)
```

### Tools with Progress Notification Support

The following tools send progress notifications during execution:

- **`flow`** ([Workflow execution](#workflow-execution)) - Reports workflow state transitions, step execution, and completion status
- **`shell_execute`** ([Shell execution](#shell-execution)) - Streams command output in real-time as the command executes
- **`search_index`** ([Semantic search](#semantic-search)) - Reports file indexing progress with counts and percentages
- **`files_glob`** ([File tools](#file-tools)) - Reports pattern matching progress across large directory trees
- **`files_grep`** ([File tools](#file-tools)) - Reports content search progress across multiple files
- **`outline_generate`** ([Code outline](#code-outline)) - Reports parsing progress across multiple source files
- **`rules_check`** ([Rules engine](#rules-engine)) - Reports rule checking progress across files
- **`web_search`** ([Web tools](#web-tools)) - Reports search execution and content fetching progress
- **`web_fetch`** ([Web tools](#web-tools)) - Reports page fetching and conversion progress

Each tool sends notifications at appropriate milestones (file batches, command output lines, workflow states) to provide responsive feedback without overwhelming the client.

For detailed information about the notification system architecture, see the [MCP Server documentation](./architecture/mcp-server.md).

## Abort Mechanism

Signal workflow termination gracefully during long-running operations.

**Tool**: `abort_create`

### Key Capabilities

- Create abort signal file with detailed reason
- Terminate long-running workflows cleanly
- Preserve workflow state for debugging and analysis
- Automatic cleanup after workflow termination
- Integration with workflow execution system

### Example Usage

```
Signal workflow abort:
"Abort the current workflow because requirements changed"

Terminate long operation:
"Create an abort signal to stop the deployment workflow"
```

## Integration with Claude Code

All features are designed for seamless integration with Claude Code:

- **Natural Language Interface**: Describe what you want in plain English
- **Context Preservation**: Long-running tasks maintain context
- **Comprehensive Tooling**: Complete development workflow support
- **Safe Operations**: Validation and error handling throughout

## Next Steps

- [Tool Catalog](./reference/tools.md): Detailed reference for all tools
- [Examples](./examples.md): Practical examples and tutorials
- [Use Cases](./use-cases.md): Best practices and patterns
- [Architecture](./architecture.md): System design and internals
