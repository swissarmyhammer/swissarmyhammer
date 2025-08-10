<div align="center">

<img src="icon.png" alt="SwissArmyHammer" width="256" height="256">

# SwissArmyHammer

**Program all the things, just by writing markdown. Really.**

📚 **[Complete Documentation & Guides](https://swissarmyhammer.github.io/swissarmyhammer)** 📚

🦀 **[Rust API Documentation](https://docs.rs/swissarmyhammer)** 🦀

[![CI](https://github.com/swissarmyhammer/swissarmyhammer/workflows/CI/badge.svg)](https://github.com/swissarmyhammer/swissarmyhammer/actions)
[![License](https://img.shields.io/badge/License-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://github.com/anthropics/model-context-protocol)

[📖 Documentation](https://swissarmyhammer.github.io/swissarmyhammer) • [🦀 API Docs](https://docs.rs/swissarmyhammer)

</div>

---

## ✨ What is SwissArmyHammer?

SwissArmyHammer transforms how you work with AI prompts and workflows by letting you manage them as simple markdown files.

- a command line app that uses Claude Code as a sub agent
- a powerful Model Context Protocol (MCP) server that seamlessly integrates with Claude Code
- a flexible Rust library for building prompt-based applications.

## TLDR

Follow the [Calcutron](https://github.com/swissarmyhammer/calcutron) sample to get started.

## 🎯 Key Features

- **📁 File-based Management** - Store prompts and sub agent workflows as markdown files with YAML front matter
- **🔄 Live Reloading** - Changes are automatically detected and reloaded
- **🎨 Liquid Templates** - Use Liquid templating with variables, conditionals, loops, and custom filters to make templates and workflows
- **⚡ MCP Integration** - Works seamlessly with Claude Code via Model Context Protocol with comprehensive tool suite
- **🗂️ Organized Hierarchy** - Built-in, user, and local prompt directories with override precedence
- **🛠️ Developer Tools** - Rich CLI with diagnostics, validation, and shell completions
- **📚 Rust Library** - Use as a dependency in your own Rust projects with comprehensive API
- **🔍 Built-in Library** - 20+ ready-to-use prompts for common development tasks
- **🔧 Workflow Engine** - Advanced state-based workflow execution with Mermaid diagrams
- **📝 Issue Management** - Git-integrated issue tracking with automatic branch management
- **💾 Memoranda System** - Note-taking and knowledge management with full-text search
- **🔍 Semantic Search** - Vector-based search with TreeSitter parsing and embedding models
- **🎯 Extensible Architecture** - Plugin system and tool registry for custom functionality

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

See [https://wballard.github.io/swissarmyhammer/installation.html](https://swissarmyhammer.github.io/swissarmyhammer/installation.html) for detailed installation instructions.

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

## 🔧 MCP Tools

SwissArmyHammer provides a comprehensive suite of MCP tools for Claude Code:

### Issue Management Tools
- `issue_create` - Create new issues with automatic numbering
- `issue_list` - List all active and completed issues  
- `issue_show` - Display issue details (supports `current` and `next`)
- `issue_work` - Switch to work branch for an issue
- `issue_update` - Update issue content
- `issue_mark_complete` - Mark issues as completed
- `issue_merge` - Merge issue branches back to main
- `issue_all_complete` - Check if all issues are completed

### Memoranda (Notes) Tools
- `memo_create` - Create new memos with ULID identifiers
- `memo_list` - List all available memos
- `memo_get` - Retrieve memo by ID
- `memo_search` - Search memos by content
- `memo_update` - Update existing memo content
- `memo_delete` - Delete memos
- `memo_get_all_context` - Get all memos for AI context

### Semantic Search Tools
- `search_index` - Index files for semantic search using TreeSitter
- `search_query` - Perform semantic search with vector similarity

All tools integrate seamlessly with Claude Code's MCP protocol and provide structured, typed responses.

## 📖 Documentation

- **[Installation Guide](https://swissarmyhammer.github.io/swissarmyhammer/installation.html)** - All installation methods
- **[Quick Start](https://swissarmyhammer.github.io/swissarmyhammer/quick-start.html)** - Get up and running
- **[Creating Prompts](https://swissarmyhammer.github.io/swissarmyhammer/creating-prompts.html)** - Prompt creation guide
- **[Claude Code Integration](https://swissarmyhammer.github.io/swissarmyhammer/claude-code-integration.html)** - Setup with Claude Code
- **[Built-in Prompts](https://swissarmyhammer.github.io/swissarmyhammer/builtin-prompts.html)** - Ready-to-use prompts

### Development Setup

See [https://wballard.github.io/swissarmyhammer/installation.html](https://swissarmyhammer.github.io/swissarmyhammer/installation.html) for development setup instructions.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/) and the [rmcp](https://github.com/rockerBOO/rmcp) MCP framework
- Inspired by the [Model Context Protocol](https://github.com/anthropics/model-context-protocol)
- Documentation powered by [mdBook](https://rust-lang.github.io/mdBook/)

---

<div align="center">

**[⭐ Star this repo](https://github.com/swissarmyhammer/swissarmyhammer/stargazers)** if you find SwissArmyHammer useful!

</div>
