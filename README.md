<div align="center">

<img src="icon.png" alt="SwissArmyHammer" width="256" height="256">

# SwissArmyHammer

**Program all the things, just by writing markdown. Really.**

📚 **[Complete Documentation & Guides](https://swissarmyhammer.github.io/swissarmyhammer)** 📚

[![CI](https://github.com/swissarmyhammer/swissarmyhammer/workflows/CI/badge.svg)](https://github.com/swissarmyhammer/swissarmyhammer/actions)
[![License](https://img.shields.io/badge/License-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://github.com/anthropics/model-context-protocol)

[📖 Documentation](https://swissarmyhammer.github.io/swissarmyhammer)

</div>

---

## ✨ What is SwissArmyHammer?

**SwissArmyHammer transforms AI prompt and workflow management by treating them as simple markdown files.**

### The Problem
Working with AI assistants involves repetitive prompt crafting, context loss, inconsistent results, limited automation, and poor organization of prompts scattered across different tools.

### The Solution
SwissArmyHammer provides a unified, file-based approach with three integrated components:

- **Command Line Application** - A powerful CLI that uses Claude Code as a sub-agent for executing prompts and workflows
- **MCP Server** - Seamless integration with Claude Code via the Model Context Protocol, providing a comprehensive tool suite  
- **Rust Library** - A flexible library for building prompt-based applications with comprehensive APIs

## TLDR

Install and get started:
```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer swissarmyhammer-cli
claude mcp add --scope user sah sah serve
```

## 🎯 Key Features

- **📁 File-based Management** - Store prompts and sub agent workflows as markdown files with YAML front matter
- **🔄 Live Reloading** - Changes are automatically detected and reloaded
- **🎨 Liquid Templates** - Use Liquid templating with variables, conditionals, loops, and custom filters to make templates and workflows
- **⚡ MCP Integration** - Works seamlessly with Claude Code via Model Context Protocol with comprehensive tool suite
- **🗂️ Organized Hierarchy** - Built-in, user, and local prompt directories with override precedence
- **🛠️ Developer Tools** - Rich CLI with diagnostics, validation, and shell completions
- **📚 Rust Library** - Use as a dependency in your own Rust projects with comprehensive API
- **🔍 Built-in Prompts** - 20+ ready-to-use prompts for common development tasks
- **🔧 Workflow Engine** - Advanced state-based workflow execution with Mermaid diagrams
- **📝 Issue Management** - Git-integrated issue tracking with automatic branch management
- **💾 Memoranda System** - Note-taking and knowledge management with full-text search
- **🔍 Semantic Search** - Vector-based search with TreeSitter parsing and embedding models
- **⚡ Dynamic CLI** - CLI commands automatically generated from MCP tools, eliminating code duplication

### Common Commands

```bash
# Get help
sah --help

# Run as MCP server (default when invoked via stdio)
sah serve

# Check configuration and diagnose issues
sah doctor

# Manage prompts
sah prompt list
sah prompt test my-prompt --var task="help me"

# Execute workflows
sah flow run my-workflow

# Issue management (automatically generated from MCP tools)
sah issue list
sah issue create --name "feature-xyz" --content "# Feature XYZ\n\nImplement new feature"
sah issue work feature-xyz

# Memoranda (notes) management (automatically generated from MCP tools)
sah memo list
sah memo create --title "Meeting Notes" --content "# Team Meeting\n\n- Discussed roadmap"

# Semantic search (automatically generated from MCP tools)
sah search index --patterns "**/*.rs"
sah search query --query "error handling"

# File operations (automatically generated from MCP tools)
sah files read --absolute-path ./src/main.rs
sah files write --file-path ./output.txt --content "Hello World"

# Validate configurations
sah validate
```

### Standard Locations

1. **Builtin** - Embedded in the SwissArmyHammer binary
   - Pre-installed prompts and workflows for common tasks
   - Always available, no setup required

2. **User** - Your personal collection
   - Prompts: `~/.swissarmyhammer/prompts/`
   - Workflows: `~/.swissarmyhammer/workflows/`
   - Shared across all your projects

3. **Local** - Project-specific files
   - Prompts: `./.swissarmyhammer/prompts/`
   - Workflows: `./.swissarmyhammer/workflows/`
   - Searched in current directory and parent directories
   - Perfect for project-specific customizations

### Example Structure

```
~/.swissarmyhammer/          # User directory
├── prompts/
│   ├── code-review.md       # Personal code review prompt
│   └── daily-standup.md     # Your daily standup template
├── workflows/
│   └── release-process.md   # Your release workflow
├── memoranda/               # Personal notes and documentation
│   ├── project-notes.md
│   └── meeting-logs.md
├── issues/                  # Issue tracking (managed automatically)
│   ├── active/
│   └── complete/
└── search.db               # Semantic search index (auto-generated)

./my-project/                # Project directory
└── .swissarmyhammer/        # Local directory
    ├── prompts/
    │   └── api-docs.md      # Project-specific API documentation prompt
    ├── workflows/
    │   └── ci-cd.md         # Project CI/CD workflow
    ├── memoranda/           # Project-specific notes
    │   └── architecture.md
    └── issues/              # Project issues
        ├── active/
        └── complete/
```

## 🚀 Quick Start

### Install

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer swissarmyhammer-cli
```

See [installation guide](https://swissarmyhammer.github.io/swissarmyhammer/installation.html) for detailed instructions.

### Configure Claude Code

Add to your Claude Code [MCP configuration](https://docs.anthropic.com/en/docs/claude-code/mcp)

```bash
claude mcp add --scope user sah sah serve
```

### Create Your First Prompt

```bash
mkdir -p ~/.swissarmyhammer/prompts
cat > ~/.swissarmyhammer/prompts/helper.md << 'EOF'
---
title: Task Helper
description: Helps with various tasks
arguments:
  - name: task
    description: What you need help with
    required: true
---

Please help me with: {{task}}

Provide clear, actionable advice.
EOF
```

That's it! Your prompt is now available in Claude Code. You can use it via MCP with `/helper`.

### Try a Built-in Workflow

SwissArmyHammer comes with built-in workflows. Try the hello-world example:

```bash
sah flow run hello-world
```

This simple workflow demonstrates:
- Basic state transitions
- Prompt execution with templating
- Variable passing between states

You can also run it through Claude Code using the MCP integration to see how workflows integrate with AI interactions.

## 🔧 MCP Tools & Dynamic CLI

SwissArmyHammer features a **dynamic CLI architecture** that automatically generates command-line interfaces from MCP tool definitions. This eliminates code duplication and ensures perfect consistency between MCP and CLI interfaces.

### Available Tool Categories

- **Issue Management** - Complete issue tracking with Git branch integration (`sah issue create`, `sah issue work`, etc.)
- **Memoranda System** - Note-taking and knowledge management (`sah memo create`, `sah memo search`, etc.)
- **File Operations** - Comprehensive file manipulation (`sah files read`, `sah files write`, `sah files grep`, etc.)
- **Semantic Search** - Vector-based code and content search (`sah search index`, `sah search query`, etc.)
- **Web Tools** - Web fetching and search capabilities (`sah web fetch`, `sah web search`, etc.)
- **Shell Integration** - Safe shell command execution (`sah shell execute`, etc.)
- **Todo Management** - Ephemeral task tracking (`sah todo create`, `sah todo show`, etc.)
- **Workflow Control** - Abort and notification tools (`sah abort create`, `sah notify create`, etc.)

### Dynamic Architecture Benefits

- **Single Source of Truth** - MCP tool schemas drive both MCP and CLI interfaces
- **Automatic CLI Generation** - New MCP tools appear in CLI without code changes  
- **Consistent Help Text** - Tool descriptions automatically become CLI help
- **Zero Maintenance** - Adding tools requires no CLI-specific code
- **Perfect Consistency** - CLI and MCP interfaces never drift apart

All tools integrate seamlessly with Claude Code's MCP protocol and provide structured, typed responses. The system uses JSON Schema to automatically generate appropriate CLI arguments, validation, and help text.

For detailed information about the architecture, see [`docs/dynamic-cli-architecture.md`](docs/dynamic-cli-architecture.md).


