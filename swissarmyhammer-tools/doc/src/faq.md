# Frequently Asked Questions

Common questions and answers about SwissArmyHammer Tools.

## General Questions

### What is SwissArmyHammer Tools?

SwissArmyHammer Tools is an MCP (Model Context Protocol) server that provides AI assistants with structured access to development tools. It enables AI assistants like Claude to work with codebases, manage workflows, and automate software engineering tasks.

### What is MCP?

MCP (Model Context Protocol) is a standard protocol for AI assistants to interact with external tools and data sources. It provides a consistent interface for tool discovery, execution, and error handling.

### Do I need to know MCP to use SwissArmyHammer Tools?

No. If you're using Claude Desktop or similar MCP clients, they handle the MCP protocol for you. You simply interact naturally with the assistant, and it uses the tools behind the scenes.

### Is SwissArmyHammer Tools free?

Yes, SwissArmyHammer Tools is open source software. Check the license for details.

## Installation and Setup

### What are the system requirements?

- Rust 1.70 or later
- Modern operating system (macOS, Linux, Windows)
- 2GB RAM minimum, 4GB recommended
- 500MB disk space

### How do I install SwissArmyHammer Tools?

The simplest way is:
```bash
cargo install swissarmyhammer
```

See [Installation](./installation.md) for more details.

### Can I use SwissArmyHammer Tools without Claude Desktop?

Yes. You can use it with any MCP client, or integrate it into your own applications using the Rust library.

### How do I update to the latest version?

```bash
cargo install swissarmyhammer --force
```

## Usage Questions

### How do I know what tools are available?

Ask your AI assistant "What SwissArmyHammer tools are available?" or see the [Tool Catalog](./tool-catalog.md).

### Do I need to index my codebase before searching?

Yes. Use the `search_index` tool first to create the search index, then use `search_query` to search.

### Why are my search results stale?

The search index is incremental by default. Force a re-index using `search_index` with `force: true` after major refactoring.

### Can I search across multiple languages?

Yes. The semantic search supports Rust, Python, TypeScript, JavaScript, and Dart simultaneously.

### How do I create an issue?

Use the `issue_create` tool with markdown content. The issue is stored as a file in `.swissarmyhammer/issues/`.

### Should I commit .swissarmyhammer to git?

Partially. Commit `issues/` and `memos/` directories, but add `search.db` and `todo.yaml` to `.gitignore`.

## Technical Questions

### Where is data stored?

SwissArmyHammer stores data in `.swissarmyhammer/`:
- Issues: `.swissarmyhammer/issues/*.md`
- Memos: `.swissarmyhammer/memos/*.md`
- Search index: `.swissarmyhammer/search.db`
- Todos: `.swissarmyhammer/todo.yaml`

### Is the search index portable?

No. The search.db file contains embeddings specific to the codebase. Don't commit it to version control. Each developer should create their own index.

### How does semantic search work?

Files are parsed with tree-sitter, chunked into meaningful segments, converted to vector embeddings, and stored in SQLite. Queries are embedded and matched using vector similarity.

### What tree-sitter grammars are supported?

- Rust
- Python
- TypeScript
- JavaScript
- Dart

### Can I add support for more languages?

Yes. Contributions are welcome. You'll need to add tree-sitter grammar support and chunking logic.

### How secure are file operations?

All file operations validate paths are within the working directory, preventing path traversal attacks. Operations are atomic to prevent data corruption.

### Can SwissArmyHammer access files outside the project?

No. All file operations are restricted to the working directory for security.

### Does SwissArmyHammer send data externally?

Only the web tools (`web_fetch` and `web_search`) make external network requests. All other tools operate locally.

## Performance Questions

### How long does indexing take?

- Small projects (1,000 lines): < 1 second
- Medium projects (10,000 lines): 1-5 seconds
- Large projects (100,000 lines): 5-30 seconds

### How fast are search queries?

Most queries complete in under 100ms. Complex queries may take up to 500ms.

### Can I index very large codebases?

Yes, but indexing time increases with codebase size. Index only the files you need to search.

### Why is the server using a lot of memory?

Check if you're:
- Reading very large files without offset/limit
- Indexing many files simultaneously
- Running many concurrent operations

### How can I improve performance?

- Index incrementally (avoid force re-index unless needed)
- Use specific glob patterns to reduce file scanning
- Use `offset` and `limit` for large files
- Limit search results to reasonable numbers

## Troubleshooting Questions

### The server won't start. What should I do?

1. Verify installation: `sah --version`
2. Check working directory exists
3. Review error messages
4. Try debug mode: `RUST_LOG=debug sah serve`

See [Troubleshooting](./troubleshooting.md) for more help.

### Why can't Claude Desktop connect?

1. Check configuration file location
2. Verify JSON syntax (no trailing commas)
3. Ensure `sah` is in PATH
4. Restart Claude Desktop

### Search returns no results. Why?

1. Verify index exists: `ls .swissarmyhammer/search.db`
2. Check patterns match your files
3. Try broader search terms
4. Verify files are in supported languages

### File operations fail with permission errors. Why?

1. Check file permissions
2. Verify paths are within working directory
3. Ensure disk space available
4. Check for file locks

## Integration Questions

### Can I use SwissArmyHammer Tools in CI/CD?

Yes. You can run tools via the command line or integrate the Rust library into your CI scripts.

### Can I integrate with VS Code?

Not directly. SwissArmyHammer Tools is designed for MCP clients. For VS Code integration, you'd need an MCP client extension.

### Can I use SwissArmyHammer Tools with GitHub Copilot?

No. GitHub Copilot doesn't support MCP. SwissArmyHammer is designed for MCP-compatible assistants like Claude.

### Can multiple users share the same MCP server?

The stdio mode is single-user. For multi-user scenarios, use HTTP mode with appropriate authentication and authorization.

### Can I run SwissArmyHammer Tools remotely?

Yes, using HTTP mode. However, ensure proper security (HTTPS, authentication, firewall rules).

## Development Questions

### Can I create custom tools?

Yes. SwissArmyHammer uses a pluggable tool registry. Implement the `McpTool` trait and register your tool.

### Can I extend the search functionality?

Yes. The search module is designed to be extensible. You can add new language parsers or chunking strategies.

### Can I contribute to SwissArmyHammer Tools?

Yes. Contributions are welcome. See the GitHub repository for contribution guidelines.

### How do I report a bug?

Report bugs on the GitHub issues page. Include:
- SwissArmyHammer version
- Operating system
- Steps to reproduce
- Expected vs actual behavior

### How do I request a feature?

Create a feature request on GitHub issues. Describe:
- Use case
- Expected behavior
- Why it's valuable

## Comparison Questions

### How is this different from Language Server Protocol (LSP)?

LSP is for IDE integrations (code completion, diagnostics). SwissArmyHammer is for AI assistant integrations (task automation, code understanding).

### How is this different from GitHub Copilot?

Copilot focuses on code completion. SwissArmyHammer provides comprehensive development tools for AI assistants to perform complex tasks.

### How is this different from Cursor?

Cursor is an IDE. SwissArmyHammer is a tool server that works with AI assistants across different clients.

### Do I need SwissArmyHammer if I have Claude Desktop?

SwissArmyHammer extends Claude Desktop with development-specific capabilities. It's optional but provides significant value for software development.

## Best Practices Questions

### When should I create an issue?

Create issues for:
- Features to implement
- Bugs to fix
- Refactoring tasks
- Documentation work

### When should I use memos?

Use memos for:
- Research notes
- Decision documentation
- Project context
- Reference information

### How should I organize issues?

Use prefixes:
- `FEATURE_` for new features
- `BUG_` for bug fixes
- `REFACTOR_` for refactoring
- `DOCS_` for documentation

### Should I commit search.db?

No. Add `.swissarmyhammer/search.db` to `.gitignore`. Each developer should create their own index.

### How often should I re-index?

- Automatic: The tool handles incremental indexing
- Manual: Force re-index after major refactoring

## Need More Help?

- [Getting Started](./getting-started.md): Installation and setup
- [Features](./features.md): Tool capabilities
- [Troubleshooting](./troubleshooting.md): Problem solving
- [GitHub Issues](https://github.com/swissarmyhammer/swissarmyhammer-tools/issues): Report bugs or request features
