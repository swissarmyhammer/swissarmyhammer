---
name: Help
title: Help and Usage Guide
description: Provides comprehensive help and usage information for Swiss Army Hammer
category: basic
tags:
  - help
  - usage
  - guide
  - documentation
arguments:
  - name: topic
    description: Specific help topic to focus on (optional)
    required: false
    default: general
---

# Swiss Army Hammer Help

## Overview
Swiss Army Hammer is a flexible prompt management library and CLI tool for AI assistants.

## Key Features
- **Prompt Management**: Load, store, and organize prompts from various sources
- **Template Engine**: Powerful Liquid-based template processing with custom filters
- **Search**: Full-text and semantic search capabilities for finding prompts
- **MCP Support**: Model Context Protocol server integration
- **Workflows**: State-based execution system for complex tasks
- **Issue Management**: Built-in issue tracking and management

## Basic Usage

### CLI Commands
- `sah list` - List available prompts
- `sah run <prompt>` - Run a specific prompt
- `sah search <query>` - Search for prompts
- `sah serve` - Start MCP server
- `sah doctor` - Run diagnostics

### Template Variables
You can use these variables in your prompts:
- Variable substitution using double braces
- Built-in variables like project_name
- Environment variables (when available)
- Custom arguments defined in prompt front matter

### Custom Filters (Available in TemplateEngine)
Swiss Army Hammer provides additional filters for text processing when using the TemplateEngine:
- `slugify` - Convert text to URL-friendly slug
- `count_lines` - Count lines in text
- `indent: N` - Indent text by number of spaces

{% if topic == "workflows" %}
## Workflows
Workflows allow you to create state-based execution flows with transitions, conditions, and actions.
{% elsif topic == "search" %}
## Search
Use full-text search or semantic search to find relevant prompts in your library.
{% endif %}

## Getting Help
- Use `sah doctor` to diagnose configuration issues
- Check the documentation at https://docs.swissarmyhammer.dev
- View built-in examples with `sah list --category examples`

For more specific help on any topic, you can run this prompt with different topic parameters.