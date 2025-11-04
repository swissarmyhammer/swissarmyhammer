# MCP Tools Overview

SwissArmyHammer Tools provides 28 MCP tools organized into logical categories for AI-assisted development.

## Tool Categories

### File Operations (5 tools)
Essential file system operations with security validation.

- **[files_read](files/read.md)** - Read file contents with partial reading support
- **[files_write](files/write.md)** - Write files atomically with encoding detection
- **[files_edit](files/edit.md)** - Precise string replacements with atomic operations
- **[files_glob](files/glob.md)** - Fast pattern matching with .gitignore support
- **[files_grep](files/grep.md)** - Content search with ripgrep

### Semantic Search (2 tools)
Vector-based code search using tree-sitter parsing.

- **[search_index](search/index.md)** - Index files for semantic search
- **[search_query](search/query.md)** - Query indexed code by meaning

### Issue Management (6 tools)
Git-friendly work item tracking as markdown files.

- **[issue_create](issues/create.md)** - Create new issues
- **[issue_list](issues/list.md)** - List all issues with filtering
- **[issue_show](issues/show.md)** - Show issue details
- **[issue_update](issues/update.md)** - Update issue content
- **[issue_mark_complete](issues/mark-complete.md)** - Mark issue complete
- **[issue_all_complete](issues/all-complete.md)** - Check completion status

### Memos (4 tools)
Note-taking and knowledge management system.

- **[memo_create](memo/create.md)** - Create new memo
- **[memo_get](memo/get.md)** - Retrieve memo by title
- **[memo_list](memo/list.md)** - List all memos
- **[memo_get_all_context](memo/get-all-context.md)** - Get all memo content for AI context

### Todo Management (3 tools)
Ephemeral task tracking for development sessions.

- **[todo_create](todo/create.md)** - Create todo item
- **[todo_show](todo/show.md)** - Show todo item
- **[todo_mark_complete](todo/mark-complete.md)** - Mark todo complete

### Git Operations (1 tool)
Version control integration.

- **[git_changes](git/changes.md)** - List changed files on a branch

### Shell Execution (1 tool)
Safe command execution with output handling.

- **[shell_execute](shell/execute.md)** - Execute shell commands

### Code Analysis (2 tools)
Code structure and quality analysis.

- **[outline_generate](outline/generate.md)** - Generate structured code outlines
- **[rules_check](rules/check.md)** - Check code against quality rules

### Web Operations (2 tools)
Web content fetching and searching.

- **[web_fetch](web/fetch.md)** - Fetch and convert web content to markdown
- **[web_search](web/search.md)** - Search web using DuckDuckGo

### Workflow Management (2 tools)
Workflow execution and control.

- **[flow](flow/flow.md)** - Execute workflows with state management
- **[abort_create](abort/create.md)** - Signal workflow termination

## Common Patterns

### Reading and Modifying Files
```
1. files_glob - Find files matching pattern
2. files_read - Read file contents
3. files_edit - Make changes
```

### Semantic Code Search
```
1. search_index - Index codebase
2. search_query - Search by meaning
3. files_read - Read matching files
```

### Issue Workflow
```
1. issue_create - Create issue
2. todo_create - Break into tasks
3. [work on tasks]
4. issue_mark_complete - Complete issue
```

### Code Analysis
```
1. outline_generate - Understand structure
2. search_query - Find relevant code
3. rules_check - Verify quality
```

## Tool Naming Convention

All tools follow the pattern `category_action`:
- **category**: Functional area (files, search, issue, etc.)
- **action**: Operation (read, create, list, etc.)

## Next Steps

Explore specific tool categories:
- [File Operations](file-operations.md) - Detailed file tool documentation
- [Semantic Search](semantic-search.md) - Search capabilities
- [Issue Management](issue-management.md) - Issue tracking workflow
- [Examples](../examples/basic.md) - Practical usage examples
