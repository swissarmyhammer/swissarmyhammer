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

# Remove sah from your agent
sah deinit
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
- **JavaScript** -- embedded JS expression evaluation
- **Questions** -- elicitation-based Q&A for capturing decisions

**Built-in Skills:**
- **plan** -- turn specs into implementation plans on a kanban board
- **kanban** -- pick up and execute the next task
- **implement** -- work through all remaining tasks autonomously
- **review** -- code review workflow with findings as kanban cards
- **test** -- run the test suite and analyze results
- **coverage** -- analyze test coverage gaps on changed code
- **commit** -- create well-structured conventional commits
- **tdd** -- strict test-driven development discipline
- **code-context** -- semantic code search and symbol lookup
- **deduplicate** -- find and refactor duplicate code
- **shell** -- shell command execution with history, semantic search, and context/token management

See the [sah README](swissarmyhammer-cli/) for full details.

## Agent/Skill/Tool Model

SwissArmyHammer implements a three-layer architecture that enables every stage of the SDLC to work with the same quality and autonomy as Plan mode:

### The Triple Pattern

Each major SDLC stage is built on the same foundation:

- **Tool** -- Low-level MCP capability (git, kanban, code search, test runners, etc.)
- **Skill** -- User-facing interface with structured workflow
- **Agent** -- Specialized autonomous executor optimized for that specific stage

This pattern means every SDLC stage—not just planning—gets the benefits of focused workflow design and specialized agent autonomy:

| SDLC Stage | Tool | Skill | Agent | What It Does |
|------------|------|-------|-------|--------------|
| **Planning** | Kanban, Code Search | `/plan` | planner | Break specs into tasks on a kanban board |
| **Implementation** | Git, Files, Kanban | `/implement` | implementer | Execute all remaining tasks autonomously |
| **Testing** | Test runners, Coverage | `/test` | tester | Run tests, find failures, fix them |
| **Test Coverage** | Code Search, Coverage | `/coverage` | tester | Find untested code, create test cards |
| **Code Review** | Code Search, Diff, Kanban | `/review` | reviewer | Review code, create findings as cards |
| **Git Workflow** | Git, Diff | `/commit` | committer | Create well-structured conventional commits |
| **Test-Driven Dev** | Test runners, Files | `/tdd` | implementer | Enforce write-test-first discipline |
| **Code Exploration** | Code Search, Tree-sitter | `/code-context` | explore | Semantic search and symbol lookup |
| **Deduplication** | Code Search | `/deduplicate` | implementer | Find and refactor duplicate code |
| **Shell Commands** | Shell, History | `/shell` | default | Safe execution with context management (keeps verbose output separate, saves tokens) |

### Integrated Pipelines

Stages connect naturally:

- **Spec → Plan → Implement → Review → Fix**: Use `/plan` for specs, `/implement` for tasks, `/review` for findings (creates cards), then `/implement fix_review` to fix them
- **Coverage-Driven Testing**: Use `/coverage` to find untested code, which creates kanban cards that `/test` picks up
- **Quality Gates**: Use `/review`, `/test`, and `/tdd` as quality checkpoints at each stage, each feeding findings back to the implementation stage

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
