# Features

SwissArmyHammer Tools provides a comprehensive suite of MCP tools organized into logical categories. This page provides an overview of all available features with links to detailed documentation.

## File Operations

Comprehensive file system operations with security validation and atomic writes.

- **files_read**: Read file contents with partial reading support
- **files_write**: Write files with atomic operations
- **files_edit**: Perform precise string replacements
- **files_glob**: Fast file pattern matching with .gitignore support
- **files_grep**: Content-based search with ripgrep

[Learn more about File Operations →](./features/file-operations.md)

## Semantic Search

Vector-based code search using tree-sitter parsing and embeddings for intelligent code navigation.

- **search_index**: Index files for semantic search
- **search_query**: Perform semantic search across indexed files

[Learn more about Semantic Search →](./features/semantic-search.md)

## Issue Management

Track work items as markdown files with complete lifecycle support and git-friendly storage.

- **issue_create**: Create new issues
- **issue_list**: List all available issues
- **issue_show**: Display details of a specific issue
- **issue_update**: Update issue content
- **issue_mark_complete**: Mark issues as complete
- **issue_all_complete**: Check if all issues are completed

[Learn more about Issue Management →](./features/issue-management.md)

## Workflow Execution

Define and execute development workflows using YAML specifications with AI coordination.

- **flow**: Execute workflows dynamically

[Learn more about Workflow Execution →](./features/workflow-execution.md)

## Git Integration

Track file changes with branch detection, parent branch tracking, and uncommitted changes.

- **git_changes**: List files that have changed on a branch

[Learn more about Git Integration →](./features/git-integration.md)

## Code Analysis

Generate structured outlines of codebases with symbol extraction for multiple languages.

- **outline_generate**: Generate structured code overviews using Tree-sitter

[Learn more about Code Analysis →](./features/code-analysis.md)

## Web Tools

Fetch and search web content with markdown conversion and DuckDuckGo integration.

- **web_fetch**: Fetch web content and convert HTML to markdown
- **web_search**: Perform web searches using DuckDuckGo

[Learn more about Web Tools →](./features/web-tools.md)

## Shell Execution

Execute shell commands with environment control and proper output handling.

- **shell_execute**: Execute shell commands with environment variables

[Learn more about Shell Execution →](./features/shell-execution.md)

## Memoranda

Note-taking and knowledge management with ULID-based organization.

- **memo_create**: Create new memos
- **memo_list**: List all available memos
- **memo_get**: Retrieve a memo by title
- **memo_get_all_context**: Get all memo content for AI context

## Todo Management

Ephemeral task tracking for development sessions with automatic cleanup.

- **todo_create**: Add new todo items
- **todo_show**: Retrieve todo items
- **todo_mark_complete**: Mark todos as complete

## Rules Engine

Check code quality against defined standards with configurable severity levels.

- **rules_check**: Check source code against SwissArmyHammer rules

## Abort Control

Signal workflow termination with reason preservation.

- **abort_create**: Create an abort file to signal workflow termination

## Tool Statistics

- **Total Tools**: 28 tools across 11 categories
- **Supported Languages**: Rust, Python, TypeScript, JavaScript, Dart
- **Storage**: File-based (markdown) and SQLite database
- **Protocol**: Full MCP (Model Context Protocol) implementation

## Next Steps

Explore detailed documentation for each feature category to learn about specific capabilities and usage examples.
