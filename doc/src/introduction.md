# Introduction

SwissArmyHammer is a comprehensive AI prompt and workflow management system that transforms how you work with AI assistants by managing prompts and workflows as simple markdown files.

## What is SwissArmyHammer?

SwissArmyHammer provides three integrated components:

- **Command Line Application** - A powerful CLI that uses Claude Code as a sub-agent for executing prompts and workflows
- **MCP Server** - Seamless integration with Claude Code via the Model Context Protocol, providing a rich suite of tools
- **Rust Library** - A flexible library for building prompt-based applications with comprehensive APIs

## Why SwissArmyHammer?

### File-Based Management
Store prompts and workflows as markdown files with YAML front matter - no databases, no complex configuration. Everything is version-controlled and easily shared.

### Live Reloading
Changes to prompt files are automatically detected and reloaded, enabling rapid development and testing.

### Liquid Templates
Use powerful Liquid templating with variables, conditionals, loops, and custom filters to create dynamic prompts and workflows.

### MCP Integration
Works seamlessly with Claude Code through the Model Context Protocol, providing a comprehensive tool suite for AI-powered development.

### Organized Hierarchy
Built-in, user, and local prompt directories with clear precedence rules:
1. **Builtin** - Pre-installed prompts embedded in the binary
2. **User** - Personal prompts in `~/.swissarmyhammer/`
3. **Local** - Project-specific prompts in `./.swissarmyhammer/`

### Developer Experience
Rich CLI with diagnostics, validation, shell completions, and comprehensive error reporting.

## Core Features

- **📁 File-based Management** - Markdown files with YAML front matter
- **🔄 Live Reloading** - Automatic change detection and reloading
- **🎨 Liquid Templates** - Variables, conditionals, loops, and custom filters
- **⚡ MCP Integration** - Seamless Claude Code integration
- **🗂️ Organized Hierarchy** - Built-in, user, and local directories
- **🛠️ Developer Tools** - Rich CLI with diagnostics and validation
- **📚 Rust Library** - Comprehensive API for building applications
- **🔍 Built-in Prompts** - 20+ ready-to-use prompts
- **🔧 Workflow Engine** - State-based execution with Mermaid diagrams
- **📝 Issue Management** - Git-integrated issue tracking
- **💾 Memoranda System** - Note-taking with full-text search
- **🔍 Semantic Search** - Vector-based search with TreeSitter parsing

## Quick Overview

### Prompts
```markdown
---
title: Code Review Helper
description: Assists with code review tasks
arguments:
  - name: language
    description: Programming language
    required: true
  - name: file
    description: File to review
    required: true
---

Please review this {{language}} code in {{file}}:

Focus on:
- Code quality and style
- Potential bugs or issues
- Performance considerations
- Best practices
```

### Workflows
```markdown
---
name: code-review-workflow
description: Complete code review process
initial_state: analyze
---

## States

### analyze
Review the code and identify issues.

**Next**: report

### report
Generate a comprehensive review report.

**Next**: complete
```

### Usage
```bash
# Install and configure
sah doctor

# Use prompts
sah prompt test code-review --var language=rust --var file=main.rs

# Execute workflows
sah flow run code-review-workflow

# MCP integration (automatically available in Claude Code)
claude mcp add sah sah serve
```

## Next Steps

- [Installation](installation.md) - Get SwissArmyHammer installed
- [Quick Start](quick-start.md) - Your first prompt in 5 minutes
- [Configuration](configuration.md) - Customize your setup
- [Architecture Overview](architecture.md) - Understand the system design