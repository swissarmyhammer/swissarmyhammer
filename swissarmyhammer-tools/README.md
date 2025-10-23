# SwissArmyHammer Tools

> **Write specs, not code.** The only coding assistant you'll ever need.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## 📚 [Complete Documentation →](https://swissarmyhammer.github.io/swissarmyhammer-tools/)

**Read the full documentation at [swissarmyhammer.github.io/swissarmyhammer-tools](https://swissarmyhammer.github.io/swissarmyhammer-tools/)** for comprehensive guides, examples, and API reference.

---

## Table of Contents

- [Overview](#overview)
- [What Problem Does This Solve?](#what-problem-does-this-solve)
- [How It Works](#how-it-works)
- [Key Features](#key-features)
- [Quick Start](#quick-start)
- [Tool Categories](#tool-categories)
- [Architecture](#architecture)
- [Documentation](#documentation)
- [Requirements](#requirements)
- [License](#license)
- [Contributing](#contributing)
- [Related Projects](#related-projects)

---

## Overview

SwissArmyHammer Tools provides a comprehensive MCP (Model Context Protocol) server that exposes powerful AI development capabilities through standardized tools and prompts. This enables AI assistants like Claude to work effectively with codebases, manage development workflows, and automate complex software engineering tasks.

## What Problem Does This Solve?

Modern AI assistants need structured, reliable ways to interact with development environments. SwissArmyHammer solves this by:

- **Standardized Interface**: MCP protocol provides a consistent way for AI assistants to access development tools
- **Comprehensive Tooling**: Complete suite of file operations, semantic search, issue tracking, and code analysis tools
- **Workflow Management**: Built-in issue tracking, todo management, and workflow execution capabilities
- **Code Understanding**: Semantic search and outline generation help AI understand large codebases
- **Safe Operations**: All file operations include security validation and atomic operations

## How It Works

SwissArmyHammer Tools implements the Model Context Protocol (MCP) specification, exposing functionality through:

1. **MCP Tools**: Individual capabilities (file operations, search, git, etc.) exposed as MCP tools
2. **MCP Prompts**: Reusable prompt templates from the SwissArmyHammer prompt library
3. **Unified Server**: Both stdio and HTTP server modes for flexible integration
4. **Tool Registry**: Modular architecture where each tool is self-contained and independently registered

The server acts as a bridge between AI assistants and your development environment, providing structured access to:

- File system operations (read, write, edit, glob, grep)
- Semantic code search with vector embeddings
- Issue and workflow management
- Git operations and change tracking
- Code outline generation and analysis
- Shell command execution
- Web fetch and search capabilities

## Key Features

- **Complete MCP Server**: Full implementation of Model Context Protocol for AI assistant integration
- **Semantic Search**: Vector-based code search using tree-sitter parsing and embeddings for intelligent code navigation
- **Issue Management**: Track work items as markdown files with complete lifecycle support and git-friendly storage
- **File Tools**: Comprehensive file operations with security validation, atomic writes, and encoding handling
- **Code Analysis**: Generate structured outlines of codebases with symbol extraction for multiple languages
- **Workflow Execution**: Define and execute development workflows using YAML specifications with AI coordination
- **Git Integration**: Track file changes with branch detection, parent branch tracking, and uncommitted changes
- **Web Tools**: Fetch and search web content with markdown conversion and DuckDuckGo integration
- **Rules Engine**: Check code quality against defined standards with configurable severity levels
- **Shell Execution**: Execute commands with environment control and proper output handling

## Quick Start

### Installation

```bash
# Install SwissArmyHammer CLI (includes tools package)
cargo install swissarmyhammer
```

### Running the MCP Server

```bash
# Start MCP server in stdio mode (for Claude Desktop, etc.)
sah serve

# Start HTTP server on custom port
sah serve --http --port 8080

# Change working directory before starting server
sah --cwd /path/to/project serve
```

### Using as a Library

```rust
use swissarmyhammer_tools::McpServer;
use swissarmyhammer_prompts::PromptLibrary;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let library = PromptLibrary::new();
    let server = McpServer::new(library).await?;
    server.initialize().await?;

    // List available tools
    let tools = server.list_tools();
    println!("Available tools: {}", tools.len());

    Ok(())
}
```

## Tool Categories

SwissArmyHammer provides 28 tools organized into logical categories:

- **Files** (`files_*`): Read, write, edit, glob pattern matching, and grep search with security validation
- **Search** (`search_*`): Semantic code search with indexing and vector similarity using tree-sitter
- **Issues** (`issue_*`): Create, list, show, update, and complete work items with lifecycle management
- **Memos** (`memo_*`): Note-taking and knowledge management with ULID-based organization
- **Todo** (`todo_*`): Ephemeral task tracking for development sessions with automatic cleanup
- **Git** (`git_*`): Track file changes with branch detection and parent branch tracking
- **Shell** (`shell_*`): Execute shell commands with environment variables and output handling
- **Outline** (`outline_*`): Generate structured code overviews using tree-sitter for multiple languages
- **Rules** (`rules_*`): Code quality checks against defined standards with severity filtering
- **Web** (`web_*`): Fetch web content with markdown conversion and DuckDuckGo search integration
- **Flow** (`flow`): Workflow execution with AI agent coordination and state management
- **Abort** (`abort_*`): Signal workflow termination with reason preservation

## Architecture

SwissArmyHammer Tools follows a clean, modular architecture:

- **MCP Server**: Implements the Model Context Protocol specification
- **Tool Registry**: Pluggable tool system where each tool is independently registered
- **Tool Context**: Shared context providing access to storage backends and operations
- **Domain Crates**: Each feature area (issues, search, git, etc.) is a separate crate
- **Storage Backends**: Abstracted storage interfaces for issues, memos, workflows

Each tool implements the `McpTool` trait providing:
- `name()`: Unique identifier
- `description()`: Human-readable documentation
- `schema()`: JSON schema for parameters
- `execute()`: Async implementation

## Documentation

### Core Documentation

- **[Getting Started](https://swissarmyhammer.github.io/swissarmyhammer-tools/01-getting-started/introduction.html)**: Installation, quick start, and configuration
- **[Architecture Overview](https://swissarmyhammer.github.io/swissarmyhammer-tools/02-concepts/architecture.html)**: System design and component relationships
- **[Features Overview](https://swissarmyhammer.github.io/swissarmyhammer-tools/02-concepts/features-overview.html)**: Detailed feature descriptions and capabilities

### Tool Documentation

- **[MCP Tools Overview](https://swissarmyhammer.github.io/swissarmyhammer-tools/05-tools/overview.html)**: All available MCP tools
- **[MCP Integration Guide](https://swissarmyhammer.github.io/swissarmyhammer-tools/05-tools/mcp-integration-guide.html)**: Comprehensive guide for using tools with Claude Code
- **[File Operations](https://swissarmyhammer.github.io/swissarmyhammer-tools/05-tools/file-tools/introduction.html)**: Read, write, edit, glob, grep
- **[Issue Management](https://swissarmyhammer.github.io/swissarmyhammer-tools/05-tools/issue-management/introduction.html)**: Git-integrated work tracking
- **[Search Operations](https://swissarmyhammer.github.io/swissarmyhammer-tools/05-tools/search-tools/introduction.html)**: Semantic code search

### Guides and Examples

- **[Basic Examples](https://swissarmyhammer.github.io/swissarmyhammer-tools/examples/basic.html)**: Simple, practical examples
- **[Advanced Examples](https://swissarmyhammer.github.io/swissarmyhammer-tools/examples/advanced.html)**: Complex integration patterns
- **[Troubleshooting](https://swissarmyhammer.github.io/swissarmyhammer-tools/07-reference/troubleshooting.html)**: Common issues and solutions
- **[CLI Reference](https://swissarmyhammer.github.io/swissarmyhammer-tools/07-reference/cli-reference.html)**: Complete command reference

## Requirements

- Rust 1.70 or later
- Modern operating system (macOS, Linux, Windows)
- Optional: mdBook for building documentation

## License

See the main SwissArmyHammer repository for license information.

## Contributing

Contributions are welcome! Please see the main SwissArmyHammer repository for contribution guidelines.

## Related Projects

- **[swissarmyhammer](https://github.com/swissarmyhammer/swissarmyhammer)**: Main CLI and orchestration
- **[swissarmyhammer-prompts](https://github.com/swissarmyhammer/swissarmyhammer-prompts)**: Prompt library and template system
- **[Model Context Protocol](https://modelcontextprotocol.io)**: MCP specification and documentation
