# SwissArmyHammer

**The MCP server for managing prompts as markdown files**

SwissArmyHammer solves the problem of AI prompt management by providing a comprehensive solution that treats prompts as first-class citizens in your development workflow. Unlike scattered prompt files or hard-coded templates, SwissArmyHammer offers a structured, versioned, and collaborative approach to prompt engineering.

## The Problem

As AI becomes central to development workflows, developers and teams face growing challenges with prompt management:

**The Prompt Chaos Problem:**
- **Scattered Everywhere**: Prompts live in scattered text files, notes apps, chat histories, and code comments - impossible to find when you need them
- **No Version Control**: Critical prompt changes disappear without trace, making it impossible to understand what worked and why
- **Copy-Paste Proliferation**: The same prompt gets duplicated and slightly tweaked across projects, creating maintenance nightmares
- **Team Isolation**: Valuable prompts remain locked in individual workflows, preventing knowledge sharing and collaboration
- **Format Anarchy**: Every project reinvents prompt organization, making it hard to move between teams or onboard new members

**The Cost of Disorganization:**
Without proper prompt management, teams waste hours recreating existing prompts, struggle to maintain consistency across projects, and lose valuable prompt engineering knowledge when team members leave.

## The Solution

SwissArmyHammer transforms prompt chaos into organized, collaborative workflow with a comprehensive management system:

**🗂️ Unified Prompt Organization**: Replace scattered prompt files with a structured, hierarchical system. Store prompts as markdown files with YAML metadata, organizing them from global built-ins to project-specific customizations.

**📝 Git-Native Workflow**: Because prompts are plain markdown files, they integrate seamlessly with your existing Git workflow. Track changes, collaborate through pull requests, and maintain a complete history of your prompt evolution.

**🔧 Powerful Template Engine**: Stop copy-pasting similar prompts. Use Liquid templating with custom filters to create dynamic, reusable prompts that adapt to different contexts and requirements.

**🤖 Claude Code Integration**: Access your entire prompt library directly in Claude Code through native MCP protocol support. No more switching between tools or hunting for that perfect prompt.

**⚡ Developer-First Tooling**: Rich CLI with instant search, validation, testing, and diagnostics ensures your prompts are always discoverable, reliable, and maintainable.

## How SwissArmyHammer Works

SwissArmyHammer transforms your prompt workflow through:

- **📁 File-based prompt management** - Store prompts as markdown files with YAML front matter
- **🔄 Live reloading** - Changes to prompt files are automatically detected and reloaded
- **🎯 Template variables** - Use `{{variable}}` syntax for dynamic prompt customization
- **⚡ MCP integration** - Works seamlessly with Claude Code and other MCP clients
- **🗂️ Organized hierarchy** - Support for built-in, user, and local prompt directories
- **🛠️ Developer-friendly** - Rich CLI with diagnostics and shell completions

## Quick Start

### Installation
```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer.git swissarmyhammer-cli
```

### Basic Usage
1. **Create a prompt directory**: `mkdir ~/.swissarmyhammer/prompts`
2. **Configure Claude Code**: Add SwissArmyHammer to your MCP configuration
3. **Create your first prompt**: Use the simple markdown + YAML format
4. **Start using prompts**: Available immediately in Claude Code

### Key Benefits

- **🔧 Zero Configuration**: Works out of the box with sensible defaults
- **📱 Cross-Platform**: Runs on macOS, Linux, and Windows
- **🔄 Real-Time Updates**: File changes are automatically detected and reloaded
- **🎯 Type Safe**: Rust implementation provides reliability and performance
- **🌐 Community Driven**: Open source with active development and contributions

### 📝 Simple Prompt Format
Create prompts using familiar markdown with YAML front matter:
```markdown
---
title: Code Review Helper
description: Helps review code for best practices and potential issues
arguments:
  - name: code
    description: The code to review
    required: true
  - name: language
    description: Programming language
    required: false
    default: "auto-detect"
---

# Code Review

Please review the following {{language}} code:

```
{{code}}
```

Focus on:
- Code quality and readability
- Potential bugs or security issues
- Performance considerations
- Best practices adherence
```

### 🎯 Template Variables
Use template variables to make prompts dynamic and reusable:
- `{{variable}}` - Required variables
- `{{variable:default}}` - Optional variables with defaults
- Support for strings, numbers, booleans, and JSON objects

### 🔧 Built-in Diagnostics
The `doctor` command helps troubleshoot setup issues:
```bash
swissarmyhammer doctor
```

## Who Should Use SwissArmyHammer?

### Development Teams
- **Standardize prompts** across projects and team members
- **Version control** prompt changes with Git integration
- **Code review** prompt modifications like any other code
- **Share libraries** of tested, proven prompts

### Individual Developers
- **Organize personal prompts** in a structured hierarchy
- **Reuse prompts** across different projects and contexts
- **Build expertise** through curated prompt collections
- **Integrate seamlessly** with existing development workflows

### Content Creators & Researchers
- **Manage specialized prompts** for specific domains
- **Create template libraries** for common content types
- **Collaborate effectively** on prompt development
- **Maintain quality** through validation and testing

### Students & Educators
- **Learn prompt engineering** through structured examples
- **Build knowledge bases** of educational prompts
- **Share resources** with classmates and colleagues
- **Track progress** through versioned prompt evolution

## Architecture

SwissArmyHammer follows a simple but powerful architecture:

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Claude Code   │◄──►│ SwissArmyHammer  │◄──►│ Prompt Files    │
│   (MCP Client)  │    │   (MCP Server)   │    │ (.md files)     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │
                              ▼
                       ┌──────────────────┐
                       │  File Watcher    │
                       │ (Auto-reload)    │
                       └──────────────────┘
```

## Next Steps

1. **[Install SwissArmyHammer](./installation.md)** - Get up and running quickly
2. **[Create Your First Prompt](./first-prompt.md)** - Learn the basics
3. **[Integrate with Claude Code](./claude-code-integration.md)** - Connect to your AI assistant
4. **[Explore Advanced Features](./advanced-prompts.md)** - Unlock the full potential

## Why Choose SwissArmyHammer?

**Proven Architecture**: Built on well-tested technologies like Rust, Liquid templating, and the MCP protocol.

**Active Development**: Regular updates, bug fixes, and new features based on community feedback.

**Comprehensive Documentation**: Detailed guides, examples, and API reference to get you productive quickly.

**Open Source**: MIT licensed with a welcoming community for contributions and feedback.

## Join the Community

- **[GitHub Repository](https://github.com/swissarmyhammer/swissarmyhammer)** - Source code, issues, and discussions
- **[Contributing Guide](./contributing.md)** - How to contribute to the project
- **[Issue Tracker](https://github.com/swissarmyhammer/swissarmyhammer/issues)** - Report bugs and request features
- **[Discussions](https://github.com/swissarmyhammer/swissarmyhammer/discussions)** - Community Q&A and sharing

## License

SwissArmyHammer is open source software licensed under the MIT License. See the [License](./license.md) page for details.