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
- **⚙️ Flexible Configuration** - Multi-format configuration (TOML, YAML, JSON) with environment variables and precedence rules
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

## ⚙️ Configuration System

SwissArmyHammer uses a powerful, multi-format configuration system that supports TOML, YAML, and JSON formats with proper precedence handling and environment variable substitution.

### Configuration File Discovery

SwissArmyHammer automatically discovers configuration files in the following locations and formats:

**Supported file names:**
- `sah.{toml,yaml,yml,json}`
- `swissarmyhammer.{toml,yaml,yml,json}`

**Search locations (in precedence order):**
1. **Global Configuration** - `~/.swissarmyhammer/`
2. **Project Configuration** - `./.swissarmyhammer/`

### Precedence Order

Configuration values are merged with the following precedence (later sources override earlier ones):

1. **Default values** (built into the application)
2. **Global config files** (`~/.swissarmyhammer/sah.*`)
3. **Project config files** (`./.swissarmyhammer/sah.*`)
4. **Environment variables** (`SAH_*` and `SWISSARMYHAMMER_*` prefixes)
5. **CLI arguments** (highest priority)

### Configuration Formats

#### TOML Example (`sah.toml`)
```toml
# Application settings
[app]
name = "MyProject"
version = "1.0.0"
debug = false

# Database configuration
[database]
host = "localhost"
port = 5432
ssl_enabled = true

[database.credentials]
username = "admin"
database = "production"

# Feature flags
[features]
experimental = false
telemetry = true

# Custom template variables
[variables]
project_root = "/path/to/project"
author = "Your Name"
```

#### YAML Example (`sah.yaml`)
```yaml
# Application settings
app:
  name: MyProject
  version: "1.0.0"
  debug: false

# Database configuration
database:
  host: localhost
  port: 5432
  ssl_enabled: true
  credentials:
    username: admin
    database: production

# Feature flags
features:
  experimental: false
  telemetry: true

# Custom template variables
variables:
  project_root: /path/to/project
  author: "Your Name"
```

#### JSON Example (`sah.json`)
```json
{
  "app": {
    "name": "MyProject",
    "version": "1.0.0",
    "debug": false
  },
  "database": {
    "host": "localhost",
    "port": 5432,
    "ssl_enabled": true,
    "credentials": {
      "username": "admin",
      "database": "production"
    }
  },
  "features": {
    "experimental": false,
    "telemetry": true
  },
  "variables": {
    "project_root": "/path/to/project",
    "author": "Your Name"
  }
}
```

### Environment Variables

Configuration values can be set via environment variables using either prefix:

```bash
# SAH_ prefix (shorter)
export SAH_APP_NAME="MyProject"
export SAH_DATABASE_HOST="localhost"
export SAH_DATABASE_PORT="5432"
export SAH_DEBUG="true"

# SWISSARMYHAMMER_ prefix (explicit)
export SWISSARMYHAMMER_APP_NAME="MyProject"
export SWISSARMYHAMMER_DATABASE_HOST="localhost"
export SWISSARMYHAMMER_DATABASE_PORT="5432"
export SWISSARMYHAMMER_DEBUG="true"
```

**Environment Variable Mapping:**
- `SAH_APP_NAME` → `app.name`
- `SAH_DATABASE_HOST` → `database.host`
- `SAH_DATABASE_CREDENTIALS_USERNAME` → `database.credentials.username`

### Environment Variable Substitution

Configuration files support environment variable substitution:

```toml
# With default values
database_url = "${DATABASE_URL:-postgresql://localhost:5432/mydb}"
api_key = "${API_KEY}"
debug = "${DEBUG:-false}"

# In nested structures
[app]
name = "${APP_NAME:-SwissArmyHammer}"
version = "${VERSION:-1.0.0}"
```

### Using Configuration in Templates

Configuration values are automatically available in all Liquid templates:

```liquid
# Application Configuration

**Project:** {{app.name}} v{{app.version}}
**Debug Mode:** {% if debug %}enabled{% else %}disabled{% endif %}

## Database Connection

```
Host: {{database.host}}:{{database.port}}
Database: {{database.credentials.database}}
SSL: {% if database.ssl_enabled %}enabled{% else %}disabled{% endif %}
```

## Features

{% for feature in features -%}
- {{feature[0] | capitalize}}: {% if feature[1] %}✓{% else %}✗{% endif %}
{% endfor %}

Connection: postgresql://{{database.credentials.username}}@{{database.host}}:{{database.port}}/{{database.credentials.database}}
```

### Configuration in Different Contexts

- **CLI Usage** - Configuration loaded automatically when using `sah` commands
- **MCP Integration** - Configuration available in all MCP tools and workflows
- **Template Rendering** - All config values accessible via `{{config.key}}` syntax
- **Workflow Execution** - Configuration merged with workflow variables

### Quick Configuration Setup

Create a basic configuration:

```bash
# Create global config directory
mkdir -p ~/.swissarmyhammer

# Create a basic TOML config
cat > ~/.swissarmyhammer/sah.toml << 'EOF'
[app]
name = "MyApp"
debug = true

[variables]
author = "Your Name"
project_type = "web"
EOF
```

Your configuration is now available in all templates and workflows!

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


