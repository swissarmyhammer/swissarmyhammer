# Introduction

**SwissArmyHammer Tools** provides a comprehensive MCP (Model Context Protocol) server that exposes powerful AI development capabilities through standardized tools. This enables AI assistants like Claude to work effectively with codebases, manage development workflows, and automate complex software engineering tasks.

## What Problem Does This Solve?

Modern AI assistants need structured, reliable ways to interact with development environments. SwissArmyHammer Tools solves this by providing:

### Standardized Interface
The Model Context Protocol (MCP) provides a consistent way for AI assistants to access development tools without requiring custom integrations for each tool or environment.

### Comprehensive Tooling
A complete suite of 28+ tools covering all aspects of AI-powered development:
- File system operations with security validation
- Semantic code search using vector embeddings
- Issue tracking with git-friendly markdown storage
- Note-taking and knowledge management
- Code analysis and outline generation
- Shell command execution with proper output handling
- Web content fetching and search capabilities

### Workflow Management
Built-in workflow execution capabilities enable AI assistants to coordinate complex multi-step tasks with proper state management and error handling.

### Code Understanding
Semantic search and outline generation help AI assistants quickly understand and navigate large codebases without reading every file.

### Safe Operations
All file operations include comprehensive security validation, atomic writes, and proper encoding handling to prevent data loss or security issues.

## How It Works

SwissArmyHammer Tools implements the Model Context Protocol (MCP) specification, exposing functionality through:

### MCP Tools
Individual capabilities exposed as MCP tools that AI assistants can invoke. Each tool has:
- A unique name (e.g., `files_read`, `search_query`, `issue_create`)
- A JSON schema defining its parameters
- Comprehensive documentation and examples
- Proper error handling and validation

### Tool Categories
Tools are organized into logical categories:
- **Files** (`files_*`): Read, write, edit, glob, grep with security validation
- **Search** (`search_*`): Semantic code search with indexing and vector similarity
- **Issues** (`issue_*`): Create, list, show, update, and complete work items
- **Memos** (`memo_*`): Note-taking and knowledge management
- **Todo** (`todo_*`): Ephemeral task tracking for development sessions
- **Git** (`git_*`): Track file changes with branch detection
- **Shell** (`shell_*`): Execute shell commands with environment control
- **Outline** (`outline_*`): Generate structured code overviews
- **Rules** (`rules_*`): Code quality checks against defined standards
- **Web** (`web_*`): Fetch and search web content
- **Flow** (`flow`): Workflow execution with AI coordination
- **Abort** (`abort_*`): Signal workflow termination

### Unified Server
The MCP server provides both stdio and HTTP server modes for flexible integration:
- **Stdio mode**: For desktop applications like Claude Code
- **HTTP mode**: For web-based integrations and remote access

### Modular Architecture
Each tool is self-contained and independently registered, making it easy to:
- Add new tools without modifying existing code
- Test tools in isolation
- Configure which tools are available
- Extend functionality with custom tools

## Key Features

- **Complete MCP Server**: Full implementation of Model Context Protocol for AI assistant integration
- **Semantic Search**: Vector-based code search using tree-sitter parsing and embeddings
- **Issue Management**: Track work items as markdown files with complete lifecycle support
- **File Tools**: Comprehensive file operations with security validation and atomic writes
- **Code Analysis**: Generate structured outlines of codebases with symbol extraction
- **Workflow Execution**: Define and execute development workflows with AI coordination
- **Git Integration**: Track file changes with branch detection and parent branch tracking
- **Web Tools**: Fetch and search web content with markdown conversion
- **Rules Engine**: Check code quality against defined standards with configurable severity
- **Shell Execution**: Execute commands with environment control and proper output handling

## Architecture at a Glance

```text
┌─────────────────────────────────────────────────────────────┐
│                    AI Assistants                            │
│              (Claude Code, Custom Clients)                  │
└─────────────────────┬───────────────────────────────────────┘
                      │ MCP Protocol (stdio/HTTP)
┌─────────────────────┴───────────────────────────────────────┐
│                    MCP Server                               │
│              (SwissArmyHammer Tools)                        │
├─────────────────────────────────────────────────────────────┤
│  Tool Registry  │  Request Handler  │  Error Management    │
└─────────────────────┬───────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
┌───────┴────┐ ┌──────┴──────┐ ┌───┴────────┐
│   File     │ │   Search    │ │   Issue    │
│   Tools    │ │   Tools     │ │   Tools    │
└────────────┘ └─────────────┘ └────────────┘
     ...           ...             ...
```text

## Use Cases

### Code Navigation and Understanding
AI assistants use semantic search and outline generation to quickly understand codebase structure and find relevant code without exhaustive file reading.

### Issue-Driven Development
Create and manage work items as markdown files, with automatic git branch management and lifecycle tracking.

### Automated Refactoring
Combine file operations, search, and validation to perform large-scale code transformations safely.

### Knowledge Management
Use memos to capture project knowledge, decisions, and context that AI assistants can reference in future sessions.

### Quality Assurance
Run rules checks to validate code against project standards and coding conventions.

### Documentation Generation
Analyze code structure and generate comprehensive documentation automatically.

## Next Steps

- **[Getting Started](getting-started.md)**: Install and configure SwissArmyHammer Tools
- **[Quick Start](quick-start.md)**: Your first MCP server in 5 minutes
- **[Architecture](architecture.md)**: Deep dive into the system design
- **[Features](features.md)**: Explore all available tools and capabilities
