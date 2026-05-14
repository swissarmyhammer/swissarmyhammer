# Command-Line Help for `kanban`

This document contains the help content for the `kanban` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/kanban-cli
```

**Command Overview:**

* [`kanban`↴](#kanban)
* [`kanban serve`↴](#kanban-serve)
* [`kanban init`↴](#kanban-init)
* [`kanban deinit`↴](#kanban-deinit)
* [`kanban doctor`↴](#kanban-doctor)
* [`kanban completion`↴](#kanban-completion)

## `kanban`

kanban - Task management for AI coding agents

Standalone CLI for SwissArmyHammer Kanban board. Exposes board, task, column, tag, and project operations as direct subcommands, and can run as an MCP server for integration with Claude Code and other agents.

**Usage:** `kanban [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `serve` — Run MCP server over stdio, exposing kanban tools
* `init` — Install kanban MCP server into Claude Code settings
* `deinit` — Remove kanban from Claude Code settings
* `doctor` — Diagnose kanban configuration and setup
* `completion` — Generate shell completion scripts

###### **Options:**

* `-d`, `--debug` — Enable debug output to stderr



## `kanban serve`

Run MCP server over stdio, exposing kanban tools

**Usage:** `kanban serve`



## `kanban init`

Install kanban MCP server into Claude Code settings

**Usage:** `kanban init [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to install the server configuration

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `kanban deinit`

Remove kanban from Claude Code settings

**Usage:** `kanban deinit [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to remove the server configuration from

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `kanban doctor`

Diagnose kanban configuration and setup

**Usage:** `kanban doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



## `kanban completion`


Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  kanban completion bash > ~/.local/share/bash-completion/completions/kanban

  # Zsh (add to ~/.zshrc or a file in fpath)
  kanban completion zsh > ~/.zfunc/_kanban

  # Fish
  kanban completion fish > ~/.config/fish/completions/kanban.fish

  # PowerShell
  kanban completion powershell >> $PROFILE


**Usage:** `kanban completion <SHELL>`

###### **Arguments:**

* `<SHELL>` — Shell to generate completion for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




