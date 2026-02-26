# sah

MCP server with skills and tools for AI coding agents. Add it to Claude Code, Cursor, or any MCP-compatible agent.

## Install

```bash
brew install swissarmyhammer/tap/swissarmyhammer-cli
```

or

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer swissarmyhammer-cli
```

Then add the MCP server:

```bash
sah init
```

Once added, every Claude Code session gets access to all sah tools and skills automatically.

## Tools

sah provides MCP tools organized by category:

### Files
| Tool | Description |
|------|-------------|
| `files` | Unified file operations (read, write, edit, glob, grep) selected via `op` parameter |

### Git
| Tool | Description |
|------|-------------|
| `git_changes` | View diffs, status, and branch information |

### Shell
| Tool | Description |
|------|-------------|
| `shell_execute` | Run shell commands with security hardening |

### Kanban
| Tool | Description |
|------|-------------|
| `kanban` | Full kanban board management -- boards, columns, tasks, subtasks, tags, dependencies |

File-backed task management. No database, just files in `.kanban/`. Supports boards, columns, swimlanes, tasks with subtasks, tags, comments, attachments, and task dependencies.

### Code Search (tree-sitter)
| Tool | Description |
|------|-------------|
| `treesitter_search` | Semantic code search across 25+ languages |
| `treesitter_query` | AST querying with S-expressions |
| `treesitter_duplicates` | Code duplication detection |
| `treesitter_status` | Index status and health |

Supports Rust, Python, TypeScript, JavaScript, Go, Java, C, C++, C#, Ruby, PHP, Swift, Kotlin, Scala, Lua, Elixir, Haskell, OCaml, Zig, Bash, HTML, CSS, JSON, YAML, TOML, Markdown, SQL, and more.

### Web
| Tool | Description |
|------|-------------|
| `web` | Fetch web pages and convert HTML to markdown |

### Workflows
| Tool | Description |
|------|-------------|
| `flow` | Discover and execute state machine workflows |

Workflows are defined in markdown with Mermaid state diagrams. They chain prompts, tool calls, and actions into multi-step orchestrations.

### Questions
| Tool | Description |
|------|-------------|
| `question_ask` | Ask the user questions via MCP elicitation |
| `question_summary` | Retrieve question/answer history |

### JavaScript
| Tool | Description |
|------|-------------|
| `js` | Evaluate JavaScript expressions (QuickJS) |

## Skills

Skills are bundles of instructions that teach the agent how to do specific tasks. They're markdown files with a name, description, and step-by-step process.

### Built-in Skills

**plan** -- Turn specs into implementation plans. Reads a spec document, explores the codebase, designs an approach, and creates a kanban board with ordered tasks and subtasks.

**kanban** -- Pick up and execute the next task from the kanban board. Moves the card to "doing", works through each subtask, runs tests, and marks the card complete.

**implement** -- Execute all remaining tasks on the kanban board autonomously. Runs the full implementation workflow end-to-end.

**commit** -- Create well-structured git commits with conventional commit messages. Cleans up scratch files, stages all relevant changes, writes a clear commit message.

**test** -- Run the project test suite and analyze results.

**tdd** -- Strict test-driven development. RED-GREEN-REFACTOR cycle with no exceptions. Includes language-specific guidance for Rust and TypeScript.

### Custom Skills

Drop a `SKILL.md` file in `~/.swissarmyhammer/skills/<name>/` or `.swissarmyhammer/skills/<name>/` in your project:

```markdown
---
name: my-skill
description: What this skill does and when to use it
allowed-tools: "*"
---

# My Skill

Instructions for the agent...
```

Skills can include additional resource files in the same directory that are loaded alongside the main SKILL.md.

## CLI Commands

```bash
sah init               # Add sah as an MCP server
sah serve              # Start MCP server (used by sah init)
sah doctor             # Check setup and diagnose issues
sah flow list          # List available workflows
sah flow <name>        # Execute a workflow
sah plan <spec>        # Shortcut for the plan workflow
sah do                 # Shortcut for the do workflow
```

## Configuration

sah looks for configuration in:
- `~/.swissarmyhammer/` -- user-level config, prompts, workflows, skills
- `.swissarmyhammer/` -- project-level config, prompts, workflows, skills

Project-level settings override user-level. Supports TOML, YAML, and JSON config files.
