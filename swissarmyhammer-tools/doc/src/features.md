# Features

SwissArmyHammer Tools provides a comprehensive suite of capabilities for AI-assisted software development. Each feature area is designed to work seamlessly with AI assistants like Claude to enhance development workflows.

## Overview

SwissArmyHammer Tools exposes functionality through MCP tools organized into logical categories:

- **[File Tools](#file-tools)**: Read, write, edit, and search files
- **[Semantic Search](#semantic-search)**: Vector-based code search and understanding
- **[Issue Management](#issue-management)**: Track work items through their lifecycle
- **[Memo System](#memo-system)**: Knowledge management and note-taking
- **[Todo Tracking](#todo-tracking)**: Ephemeral task tracking for development sessions
- **[Git Integration](#git-integration)**: Track changes and integrate with git workflows
- **[Shell Execution](#shell-execution)**: Execute commands with proper output handling
- **[Code Outline](#code-outline)**: Generate structured code overviews
- **[Rules Engine](#rules-engine)**: Check code against quality standards
- **[Web Tools](#web-tools)**: Fetch and search web content
- **[Workflow Execution](#workflow-execution)**: Execute complex development workflows

## File Tools

File operations with security validation and atomic operations.

**Tools**: `files_read`, `files_write`, `files_edit`, `files_glob`, `files_grep`

### Key Capabilities

- Read files with partial reading support (offset and limit)
- Write files atomically with automatic backup
- Perform precise string replacements with validation
- Pattern matching with glob support
- Content search with regex using ripgrep

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

## Additional Features

### Notification System

Send notifications from AI to user through the logging system.

**Tool**: `notify_create`

- Send info, warning, or error notifications
- Include structured context data
- Integration with logging infrastructure

### Abort Mechanism

Signal workflow termination gracefully.

**Tool**: `abort_create`

- Create abort file with reason
- Terminate long-running workflows
- Preserve state for debugging

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
