<div align="center">

<img src="icon.png" alt="SwissArmyHammer" width="256" height="256">

# SwissArmyHammer

**Skills and Tools for Any Agent**

[![CI](https://github.com/swissarmyhammer/swissarmyhammer/workflows/CI/badge.svg)](https://github.com/swissarmyhammer/swissarmyhammer/actions)
[![License](https://img.shields.io/badge/License-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.91+-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://github.com/anthropics/model-context-protocol)

</div>

---

SwissArmyHammer is a suite of tools that make AI coding agents better. Install skills, validators, and tools into any MCP-compatible agent -- Claude Code, Cursor, Windsurf, or your own.

## The Suite

| Tool | Binary | What it does |
|------|--------|-------------|
| [**sah**](swissarmyhammer-cli/) | `sah` | MCP server with skills and tools for any AI agent. Kanban boards, code search, workflows, git automation, and more. |
| [**avp**](https://agentvalidatorprotocol.com) | `avp` | Agent Validator Protocol. Hooks into Claude Code to run validators on every file write and edit -- code quality, security, test integrity checks that catch problems before they land. |
| [**mirdan**](https://mirdan.ai) | `mirdan` | Universal package manager for skills and validators. Install, publish, and share packages across agents from the registry. |

## Quick Start

```bash
# Install
brew install swissarmyhammer/tap/swissarmyhammer-cli

# Add sah as an MCP server and check setup
sah init
```

## sah -- Skills and Tools for Any Agent

`sah` is an MCP server. Add it to any agent and it gets a set of tools and built-in skills.

**Tools:**
- **Files** -- read, write, edit, glob, grep with .gitignore support
- **Git** -- branch, commit, diff, status, PR workflows
- **Shell** -- safe command execution with security hardening
- **Kanban** -- file-backed task boards with cards, subtasks, dependencies
- **Code Search** -- tree-sitter powered semantic search across 25+ languages
- **Web** -- fetch pages, search the web
- **Workflows** -- state machine orchestration defined in markdown
- **JavaScript** -- embedded JS expression evaluation
- **Questions** -- elicitation-based Q&A for capturing decisions

**Built-in Skills:**
- **plan** -- turn specs into implementation plans on a kanban board
- **kanban** -- pick up and execute the next task
- **implement** -- work through all remaining tasks autonomously
- **commit** -- create well-structured conventional commits
- **test** -- run the test suite and analyze results
- **tdd** -- strict test-driven development discipline

See the [sah README](swissarmyhammer-cli/) for full details.

## [avp](https://agentvalidatorprotocol.com) -- Agent Validator Protocol

`avp` hooks into Claude Code as a pre/post tool-use validator. Every time the agent writes or edits a file, validators run automatically and can block bad changes.

**Built-in validators:**
- **code-quality** -- cognitive complexity, function length, naming, magic numbers, commented code
- **security-rules** -- no secrets in code, input validation
- **command-safety** -- safe shell command checks
- **test-integrity** -- no test cheating

```bash
# Install validators into your project
avp init

# List active validators
avp list
```

## [mirdan](https://mirdan.ai) -- Package Manager

`mirdan` is a universal package manager for skills and validators. It works across agents -- install a skill once and it's available everywhere.

```bash
# Search for packages
mirdan search code-quality

# Install a package
mirdan install code-quality

# See what agents you have installed
mirdan agents

# Publish your own
mirdan publish
```

## Architecture

Everything is markdown files. Skills, validators, workflows, prompts -- all markdown with YAML frontmatter and Liquid templating. No databases, no cloud lock-in, everything version-controllable.

```
~/.swissarmyhammer/
  prompts/          # Custom prompts (markdown + Liquid)
  workflows/        # State machine workflows (markdown + Mermaid)
  skills/           # Installed skills
  validators/       # Installed validators
```

## License

MIT OR Apache-2.0
