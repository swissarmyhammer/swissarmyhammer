# Introduction

> **Write specs, not code.** The only coding assistant you'll ever need.

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

## Next Steps

- **[Getting Started](./getting-started.md)**: Install and configure SwissArmyHammer Tools
- **[Features](./features.md)**: Explore the available tools and capabilities
- **[Architecture](./architecture.md)**: Understand the system design and components
- **[Tool Catalog](./tool-catalog.md)**: Browse the complete tool reference
