# Frequently Asked Questions

## General Questions

### What is SwissArmyHammer Tools?

SwissArmyHammer Tools is an MCP (Model Context Protocol) server that provides AI assistants with a comprehensive set of tools for code analysis, file operations, issue tracking, and more. It enables AI agents to interact with your codebase effectively and safely.

### How does SwissArmyHammer differ from other MCP servers?

SwissArmyHammer is designed specifically for software development workflows, with built-in support for:
- Semantic code search using vector embeddings
- Issue and task management
- Git integration
- Safe file operations with validation
- Structured workflows and agents

### What languages and frameworks does SwissArmyHammer support?

SwissArmyHammer's semantic search and code analysis features currently support:
- Rust
- Python
- TypeScript
- JavaScript
- Dart

The file operation tools work with any text-based file format.

## Installation and Setup

### How do I install SwissArmyHammer?

Install using cargo:
```bash
cargo install swissarmyhammer
```

The `sah` command-line tool will be available after installation, which includes the MCP server and all tools.

### Where is the configuration stored?

Configuration is stored in `.swissarmyhammer/config.yaml` in your project directory. User-level configuration can be stored in `~/.config/swissarmyhammer/config.yaml`.

### Can I use SwissArmyHammer with multiple projects?

Yes. Each project has its own `.swissarmyhammer` directory for project-specific issues, memos, and configuration. You can also define user-level workflows and agents that are available across all projects.

## Usage

### How do I use SwissArmyHammer with Claude or other AI assistants?

Start the MCP server:
```bash
sah serve
```

Then configure your AI assistant to connect to the MCP server. Most AI assistants will automatically discover and use the available tools.

### Do I need to index my code before using semantic search?

Yes. Run the indexing command first:
```bash
sah search index '**/*.rs'
```

Then you can perform semantic searches:
```bash
sah search query "error handling patterns"
```

### How do file paths work with SwissArmyHammer tools?

All file operation tools require absolute paths. Relative paths are not supported. The tools will return an error if a relative path is provided.

## Issues and Workflow

### What's the difference between issues and memos?

- **Issues** are tracked tasks with status (pending/active/completed) that can be worked on using issue branches
- **Memos** are freeform notes and documentation without status tracking

### Can I use SwissArmyHammer with my existing issue tracker?

SwissArmyHammer's issue system is designed for local, ephemeral tasks during development sessions. It's meant to complement, not replace, your main issue tracker (GitHub Issues, Jira, etc.).

### How do I create custom workflows?

Create a workflow definition file in `.swissarmyhammer/workflows/` following the workflow schema. See the [Architecture documentation](./architecture.md) for details on workflow structure.

## Troubleshooting

### The MCP server won't start. What should I do?

Check the common issues:
1. Ensure the port is not already in use
2. Verify your configuration file is valid YAML
3. Check file permissions on the `.swissarmyhammer` directory
4. Review the logs for specific error messages

See the [Troubleshooting guide](./troubleshooting.md) for more details.

### Semantic search is not finding relevant code. Why?

Common causes:
1. Code hasn't been indexed yet - run `sah search index`
2. Index is stale - re-index with `--force` flag
3. Query is too specific or too general - try different search terms
4. File type not supported - check supported languages above

### File operations are failing with "path must be absolute" errors

SwissArmyHammer tools require absolute paths for all file operations. Convert relative paths to absolute:
```bash
# Instead of: sah files read ./src/main.rs
# Use: sah files read /full/path/to/src/main.rs
```

## Performance

### How much disk space does indexing require?

The search index typically uses 1-5% of your codebase size. A 100MB codebase will generate a 1-5MB index file stored in `.swissarmyhammer/search.db`.

### Can SwissArmyHammer handle large codebases?

Yes. SwissArmyHammer is designed to work efficiently with large codebases. Indexing and search operations are optimized for performance. For very large codebases (>1GB), consider indexing specific directories rather than the entire codebase.

### How do I improve search performance?

- Index only the directories you actively work with
- Use specific file type patterns when indexing
- Keep your index up to date by re-indexing after major changes
- Use more specific search queries to reduce result set size

## Security

### Is it safe to run AI-suggested commands through SwissArmyHammer?

SwissArmyHammer includes several safety features:
- Path validation prevents directory traversal attacks
- Shell command sandboxing limits access
- File operations are atomic to prevent partial writes
- All operations are logged for auditing

However, always review AI-suggested commands before execution, especially destructive operations.

### What data does SwissArmyHammer collect?

SwissArmyHammer does not collect or transmit any data. All operations are local. The only network activity is:
- Communication with the MCP client
- Optional AI model API calls (if configured)
- Web search operations (if used)

### Can I use SwissArmyHammer in a secure environment?

Yes. SwissArmyHammer can run completely offline and does not require internet access for core functionality. Web search and AI model features can be disabled if needed.

## Contributing

### How can I contribute to SwissArmyHammer?

Contributions are welcome! See the development guide and contributing guidelines in the repository.

### How do I report bugs or request features?

File issues on the GitHub repository at [swissarmyhammer/swissarmyhammer](https://github.com/swissarmyhammer/swissarmyhammer).

### Can I create custom tools for SwissArmyHammer?

Yes. SwissArmyHammer's tool system is extensible. See the [Tool Registry documentation](./architecture/tool-registry.md) for details on implementing custom tools.
