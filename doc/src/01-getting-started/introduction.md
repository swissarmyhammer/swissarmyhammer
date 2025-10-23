# Introduction

SwissArmyHammer transforms AI prompt and workflow management by treating them as simple markdown files. It provides a unified, file-based approach that integrates seamlessly with your development workflow and Claude Code.

## The Problem

Working with AI assistants involves repetitive prompt crafting, context loss, inconsistent results, limited automation, and poor organization of prompts scattered across different tools.

## The Solution

SwissArmyHammer provides three integrated components that work together to solve these problems:

### Command Line Application
A powerful CLI that executes prompts and workflows, with comprehensive diagnostics, validation, and shell completions.

### MCP Server  
Seamless integration with Claude Code via the Model Context Protocol, providing a comprehensive tool suite for AI-powered development.

### Rust Library
A flexible library for building prompt-based applications with comprehensive APIs for custom integrations.

## Core Architecture

SwissArmyHammer uses a hierarchical file system approach:

### File-Based Management
- Store prompts and workflows as markdown files with YAML front matter
- No databases or complex configuration required
- Everything is version-controlled and easily shared
- Live reloading with automatic change detection

### Organized Hierarchy
Clear precedence rules across three locations:

1. **Builtin** - Pre-installed prompts and workflows embedded in the binary
2. **User** - Personal collection in `~/.swissarmyhammer/`  
3. **Local** - Project-specific files in `./.swissarmyhammer/`

### Liquid Template Engine
- Dynamic content with variables, conditionals, and loops
- Custom filters for domain-specific operations
- Environment integration and system context access
- Extensible plugin architecture

## Key Features

**Workflow Management**
- State-based workflow execution with Mermaid diagrams
- Parallel and sequential action execution
- Built-in error handling and recovery mechanisms

**Development Integration**
- Git-integrated issue tracking with automatic branch management
- Semantic search using vector embeddings and TreeSitter parsing
- Note-taking system with full-text search capabilities

**Built-in Resources**
- 20+ production-ready prompts for common development tasks
- Example workflows demonstrating best practices
- Comprehensive MCP tool suite for Claude Code integration

## Quick Examples

### Simple Prompt
```markdown
---
title: Code Review Helper
description: Assists with code review tasks
arguments:
  - name: language
    description: Programming language
    required: true
---

Review this {{language}} code for:
- Quality and style
- Potential bugs
- Performance issues
- Best practices
```

### Basic Workflow
```markdown
---
name: feature-development
description: Complete feature development process
initial_state: plan
---

### plan
Plan the feature implementation
**Next**: implement

### implement  
Write the feature code
**Next**: review

### review
Review the implementation
**Next**: complete
```

### Command Line Usage
```bash
# Diagnose setup
sah doctor

# Test a prompt
sah prompt test code-review --var language=rust

# Run a workflow
sah flow run feature-development

# Configure Claude Code integration
claude mcp add --scope user sah sah serve
```

## Next Steps

- [Installation](installation.md) - Get SwissArmyHammer installed
- [Quick Start](quick-start.md) - Start with full auto coding in 5 minutes