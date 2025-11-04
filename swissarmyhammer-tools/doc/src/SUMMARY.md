# SwissArmyHammer Tools Documentation

[Introduction](introduction.md)

# Getting Started

- [What is SwissArmyHammer Tools?](getting-started/what-is-it.md)
- [Installation](getting-started/installation.md)
- [Quick Start](getting-started/quick-start.md)
- [Configuration](getting-started/configuration.md)

# Core Concepts

- [Architecture Overview](concepts/architecture.md)
- [MCP Server](concepts/mcp-server.md)
- [Tool Registry](concepts/tool-registry.md)
- [Storage Backends](concepts/storage-backends.md)

# MCP Tools Reference

- [Tools Overview](tools/overview.md)
- [File Operations](tools/file-operations.md)
  - [files_read](tools/files/read.md)
  - [files_write](tools/files/write.md)
  - [files_edit](tools/files/edit.md)
  - [files_glob](tools/files/glob.md)
  - [files_grep](tools/files/grep.md)
- [Semantic Search](tools/semantic-search.md)
  - [search_index](tools/search/index.md)
  - [search_query](tools/search/query.md)
- [Issue Management](tools/issue-management.md)
  - [issue_create](tools/issues/create.md)
  - [issue_list](tools/issues/list.md)
  - [issue_show](tools/issues/show.md)
  - [issue_update](tools/issues/update.md)
  - [issue_mark_complete](tools/issues/mark-complete.md)
  - [issue_all_complete](tools/issues/all-complete.md)
- [Memos](tools/memos.md)
  - [memo_create](tools/memo/create.md)
  - [memo_get](tools/memo/get.md)
  - [memo_list](tools/memo/list.md)
  - [memo_get_all_context](tools/memo/get-all-context.md)
- [Todo Management](tools/todo.md)
  - [todo_create](tools/todo/create.md)
  - [todo_show](tools/todo/show.md)
  - [todo_mark_complete](tools/todo/mark-complete.md)
- [Git Operations](tools/git.md)
  - [git_changes](tools/git/changes.md)
- [Shell Execution](tools/shell.md)
  - [shell_execute](tools/shell/execute.md)
- [Code Analysis](tools/code-analysis.md)
  - [outline_generate](tools/outline/generate.md)
  - [rules_check](tools/rules/check.md)
- [Web Operations](tools/web.md)
  - [web_fetch](tools/web/fetch.md)
  - [web_search](tools/web/search.md)
- [Workflow Management](tools/workflow.md)
  - [flow](tools/flow/flow.md)
  - [abort_create](tools/abort/create.md)

# Examples and Use Cases

- [Basic Examples](examples/basic.md)
- [File Operations Examples](examples/file-operations.md)
- [Search Examples](examples/search.md)
- [Issue Tracking Workflow](examples/issue-workflow.md)
- [Code Analysis Examples](examples/code-analysis.md)
- [Advanced Patterns](examples/advanced.md)

# Integration Guides

- [Claude Code Integration](integration/claude-code.md)
- [Using as a Library](integration/library.md)
- [HTTP Server Mode](integration/http-server.md)
- [Custom Tool Development](integration/custom-tools.md)

# Reference

- [CLI Reference](reference/cli.md)
- [Configuration Reference](reference/configuration.md)
- [Error Codes](reference/error-codes.md)
- [Troubleshooting](reference/troubleshooting.md)
- [FAQ](reference/faq.md)
