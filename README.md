<div align="center">

<img src="icon.png" alt="SwissArmyHammer" width="256" height="256">

# SwissArmyHammer

**Program all the things, just by writing markdown. Really.**

📚 **[Complete Documentation & Guides](https://wballard.github.io/sahdoc)** 📚

[![CI](https://github.com/wballard/sahdoc/workflows/CI/badge.svg)](https://github.com/wballard/sahdoc/actions)
[![License](https://img.shields.io/badge/License-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://github.com/anthropics/model-context-protocol)

[📖 Documentation](https://wballard.github.io/sahdoc)

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
cargo install --git https://github.com/wballard/sahdoc swissarmyhammer-cli
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

# Issue management
sah issue list
sah issue create --name "feature-xyz" --content "# Feature XYZ\n\nImplement new feature"
sah issue work feature-xyz

# Memoranda (notes) management
sah memo list
sah memo create --title "Meeting Notes" --content "# Team Meeting\n\n- Discussed roadmap"

# Semantic search
sah search index "**/*.rs"
sah search query "error handling"

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
cargo install --git https://github.com/wballard/sahdoc swissarmyhammer-cli
```

See [installation guide](https://wballard.github.io/sahdoc/installation.html) for detailed instructions.

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

## 🔧 MCP Tools

SwissArmyHammer provides a comprehensive suite of MCP tools for Claude Code:

- **Abort Tool** - Controlled workflow termination with file-based abort detection
- **Issue Management** - Complete issue tracking with Git branch integration  
- **Memoranda System** - Note-taking and knowledge management with search
- **Semantic Search** - Vector-based code and content search with TreeSitter parsing

All tools integrate seamlessly with Claude Code's MCP protocol and provide structured, typed responses. The abort tool provides robust workflow control, replacing legacy string-based detection with a reliable file-based approach.


---

<div align="center">

**[⭐ Star this repo](https://github.com/wballard/sahdoc/stargazers)** if you find SwissArmyHammer useful!

</div>
