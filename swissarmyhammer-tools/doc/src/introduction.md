# Introduction

SwissArmyHammer Tools provides a comprehensive MCP (Model Context Protocol) server implementation that exposes powerful AI development capabilities through standardized tools. This enables AI assistants like Claude to work effectively with codebases, manage development workflows, and automate complex software engineering tasks.

## What Problem Does This Solve?

Modern AI assistants need structured, reliable ways to interact with development environments. SwissArmyHammer Tools solves this by providing:

### Standardized Interface
The Model Context Protocol provides a consistent way for AI assistants to access development tools, eliminating the need for custom integrations.

### Comprehensive Tooling
A complete suite of 28 tools covering:
- File operations (read, write, edit, search)
- Semantic code search with vector embeddings
- Issue tracking and workflow management
- Git operations and change tracking
- Code analysis and quality checking
- Web operations for research and documentation

### Workflow Management
Built-in support for:
- Issue tracking as markdown files
- Todo management for development sessions
- Workflow execution with state management
- Progress notifications for long-running operations

### Code Understanding
Advanced capabilities including:
- Semantic search using tree-sitter parsing
- Vector-based code similarity
- Structured code outlines
- Multi-language support

### Safe Operations
Security-focused design with:
- Validated file operations
- Atomic writes and edits
- Sandboxed shell execution
- Input validation and error handling

## How It Works

SwissArmyHammer Tools implements the Model Context Protocol specification, exposing functionality through:

1. **MCP Tools**: Individual capabilities exposed as standardized MCP tools
2. **MCP Server**: Unified server supporting both stdio and HTTP modes
3. **Tool Registry**: Modular architecture where each tool is self-contained
4. **Storage Backends**: Abstracted storage for issues, memos, and workflows

The server acts as a bridge between AI assistants and your development environment, providing structured access to essential development operations.

## Key Features

- **Complete MCP Server**: Full Model Context Protocol implementation
- **Semantic Search**: Vector-based code search using tree-sitter parsing
- **Issue Management**: Git-friendly markdown-based issue tracking
- **File Tools**: Comprehensive file operations with security validation
- **Code Analysis**: Structured code outlines for multiple languages
- **Workflow Execution**: Define and execute complex development workflows
- **Git Integration**: Track changes with branch detection
- **Web Tools**: Fetch and search web content with markdown conversion
- **Rules Engine**: Check code quality against defined standards
- **Shell Execution**: Execute commands with proper output handling

## Quick Example

Here's a simple example of using SwissArmyHammer Tools as a library:

```rust
use swissarmyhammer_tools::McpServer;
use swissarmyhammer_prompts::PromptLibrary;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the prompt library
    let library = PromptLibrary::new();

    // Create the MCP server
    let server = McpServer::new(library, None).await?;

    // Initialize to register all tools
    server.initialize().await?;

    // List available tools
    let tools = server.list_tools();
    println!("Available tools: {}", tools.len());

    Ok(())
}
```

## Tool Categories

The 28 tools are organized into logical categories:

- **Files** (`files_*`): File system operations
- **Search** (`search_*`): Semantic code search
- **Issues** (`issue_*`): Work item management
- **Memos** (`memo_*`): Note-taking system
- **Todo** (`todo_*`): Task tracking
- **Git** (`git_*`): Version control operations
- **Shell** (`shell_*`): Command execution
- **Outline** (`outline_*`): Code structure analysis
- **Rules** (`rules_*`): Code quality checks
- **Web** (`web_*`): Web content operations
- **Flow** (`flow`): Workflow execution
- **Abort** (`abort_*`): Workflow control

## Next Steps

- **[What is SwissArmyHammer Tools?](getting-started/what-is-it.md)** - Learn more about the project
- **[Installation](getting-started/installation.md)** - Get started with installation
- **[Quick Start](getting-started/quick-start.md)** - Run your first commands
- **[Architecture Overview](concepts/architecture.md)** - Understand the system design
