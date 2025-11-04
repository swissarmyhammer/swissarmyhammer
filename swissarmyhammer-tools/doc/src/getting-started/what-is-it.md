# What is SwissArmyHammer Tools?

SwissArmyHammer Tools is a comprehensive MCP (Model Context Protocol) server that provides AI assistants with powerful development capabilities. It bridges the gap between AI assistants like Claude and your development environment.

## Core Purpose

SwissArmyHammer Tools enables AI assistants to:

- **Read and write files** with security validation
- **Search code semantically** using vector embeddings
- **Track work items** as markdown files
- **Execute workflows** with state management
- **Analyze code structure** across multiple languages
- **Check code quality** against defined rules
- **Interact with Git** to understand changes
- **Execute shell commands** safely
- **Fetch web content** for research

## Model Context Protocol (MCP)

MCP is an open protocol that standardizes how AI assistants interact with external tools and data sources. SwissArmyHammer Tools implements this protocol, providing:

- **Standardized tool interface**: Consistent JSON-RPC based communication
- **Schema validation**: All tools have well-defined input/output schemas
- **Progress notifications**: Real-time updates for long-running operations
- **Error handling**: Graceful degradation with clear error messages
- **Multiple transport modes**: Both stdio and HTTP server support

## Architecture

SwissArmyHammer Tools follows a clean, modular architecture:

```
┌─────────────────────────────────────────┐
│         AI Assistant (Claude)           │
└────────────────┬────────────────────────┘
                 │ MCP Protocol
┌────────────────▼────────────────────────┐
│         MCP Server                      │
│  ┌──────────────────────────────────┐  │
│  │      Tool Registry               │  │
│  │  - File Tools                    │  │
│  │  - Search Tools                  │  │
│  │  - Issue Tools                   │  │
│  │  - Workflow Tools                │  │
│  └──────────────────────────────────┘  │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│     Domain Crates                       │
│  - swissarmyhammer-issues               │
│  - swissarmyhammer-search               │
│  - swissarmyhammer-git                  │
│  - swissarmyhammer-workflow             │
└─────────────────────────────────────────┘
```

## Key Components

### MCP Server
The central server component that:
- Handles MCP protocol communication
- Routes tool calls to appropriate handlers
- Manages tool registration and validation
- Provides progress notifications

### Tool Registry
A pluggable system where each tool:
- Implements the `McpTool` trait
- Defines its own JSON schema
- Handles its own execution logic
- Reports errors consistently

### Tool Context
Shared context providing:
- Working directory management
- Storage backend access
- Configuration settings
- Logging infrastructure

### Storage Backends
Abstracted storage for:
- Issues (`.swissarmyhammer/issues/`)
- Memos (`.swissarmyhammer/memos/`)
- Todos (`.swissarmyhammer/todo/`)
- Workflows (`.swissarmyhammer/workflows/`)

## Use Cases

### AI-Powered Development
- Let Claude read, understand, and modify your codebase
- Semantic code search to find relevant functionality
- Automated code analysis and quality checks

### Workflow Automation
- Define multi-step workflows as markdown
- Execute complex development processes
- Track progress with issue management

### Code Understanding
- Generate structured outlines of large codebases
- Search by meaning, not just keywords
- Analyze code across multiple languages

### Project Management
- Track issues as git-friendly markdown files
- Create and manage development todos
- Document decisions with memos

## Why MCP?

The Model Context Protocol provides:

1. **Standardization**: Consistent interface across tools
2. **Composability**: Tools can be combined and chained
3. **Extensibility**: Easy to add new tools
4. **Reliability**: Well-defined error handling
5. **Interoperability**: Works with any MCP-compatible client

## Next Steps

Ready to get started? Continue with:

- [Installation](installation.md) - Install SwissArmyHammer Tools
- [Quick Start](quick-start.md) - Run your first commands
- [Configuration](configuration.md) - Configure for your environment
