# Introduction

Welcome to SwissArmyHammer Tools, a comprehensive MCP (Model Context Protocol) server that enables AI assistants to work effectively with development environments.

## What is SwissArmyHammer Tools?

SwissArmyHammer Tools is the MCP server component of the SwissArmyHammer ecosystem. It exposes powerful development capabilities through a standardized protocol that AI assistants like Claude can use to:

- Read, write, and edit files with atomic operations and security validation
- Search code semantically using vector embeddings and tree-sitter parsing
- Manage issues and track work items through their complete lifecycle
- Execute shell commands with proper output handling and environment control
- Analyze code structure and generate outlines for multiple languages
- Track changes with git integration and branch-based workflows
- Execute complex workflows with AI agent coordination and state management
- Fetch and search web content with markdown conversion
- Check code quality against defined rules and standards

## The SwissArmyHammer Philosophy

**Write specs, not code.** SwissArmyHammer is built on the principle that developers should focus on describing what they want, not how to implement it. The AI assistant handles the implementation details, using the comprehensive toolset provided by SwissArmyHammer Tools.

## Why MCP?

The Model Context Protocol (MCP) provides a standardized way for AI assistants to access tools and resources. This means:

- **Interoperability**: Any MCP-compatible AI assistant can use SwissArmyHammer Tools
- **Consistency**: Tools have well-defined interfaces and behaviors
- **Extensibility**: New tools can be added without breaking existing functionality
- **Safety**: Tools can implement proper validation and security measures

## Core Concepts

### Tools

Tools are individual capabilities exposed through the MCP protocol. Each tool:
- Has a unique name and description
- Defines a JSON schema for its parameters
- Implements async execution logic
- Returns structured results

### Prompts

Prompts are reusable templates from the SwissArmyHammer prompt library. They provide:
- Consistent instructions for common tasks
- Template variables for customization
- Documentation co-located with code

### Tool Context

The tool context provides shared access to:
- Storage backends (issues, memos, workflows)
- Git operations
- Configuration and settings
- Shared utilities

### Storage Backends

Storage backends provide persistent data management:
- **Issue Storage**: Track work items as markdown files
- **Memo Storage**: Knowledge management and note-taking
- **Workflow Storage**: Define and execute development workflows

## Use Cases

SwissArmyHammer Tools excels at:

- **Code Navigation**: Semantic search helps find relevant code across large codebases
- **Issue Management**: Track work items with complete lifecycle support
- **Refactoring**: Find and replace code patterns across multiple files
- **Documentation**: Generate outlines and understand code structure
- **Workflow Automation**: Execute complex multi-step development workflows
- **Integration**: Connect AI assistants to your development environment

## Next Steps

- [Getting Started](./getting-started.md): Install and configure SwissArmyHammer Tools
- [Features](./features.md): Explore the available tools and capabilities
- [Architecture](./architecture.md): Understand the system design
