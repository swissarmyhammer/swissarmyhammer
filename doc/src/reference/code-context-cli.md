# Command-Line Help for `code-context`

This document contains the help content for the `code-context` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/code-context-cli
```

**Command Overview:**

* [`code-context`↴](#code-context)
* [`code-context serve`↴](#code-context-serve)
* [`code-context init`↴](#code-context-init)
* [`code-context deinit`↴](#code-context-deinit)
* [`code-context doctor`↴](#code-context-doctor)
* [`code-context skill`↴](#code-context-skill)
* [`code-context completion`↴](#code-context-completion)

## `code-context`

code-context - Structural code intelligence for AI agents

Provides indexed code navigation, symbol lookup, call graph traversal, blast radius analysis, and semantic search. Exposes these capabilities as MCP tools for AI coding agents.

**Usage:** `code-context [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `serve` — Run MCP server over stdio, exposing code-context tools
* `init` — Install code-context MCP server into Claude Code settings
* `deinit` — Remove code-context from Claude Code settings
* `doctor` — Diagnose code-context configuration and setup
* `skill` — Deploy code-context skill to agent .skills/ directories
* `completion` — Generate shell completion scripts

###### **Options:**

* `-d`, `--debug` — Enable debug output to stderr
* `-j`, `--json` — Output results as JSON (for operation commands)
* `--no-progress` — Disable interactive progress bars for long-running operations.

   `indicatif` auto-degrades to plain output on non-TTY stdout, but some environments (CI runners, recording wrappers) still benefit from a hard switch. With this flag set the dispatcher installs a no-op renderer and the tool emits no progress chrome.



## `code-context serve`

Run MCP server over stdio, exposing code-context tools

**Usage:** `code-context serve`



## `code-context init`

Install code-context MCP server into Claude Code settings

**Usage:** `code-context init [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to install the server configuration

  Default value: `project`

  Possible values:
  - `project`:
    Project-level configuration (committed to the repo)
  - `local`:
    Local project configuration that is not committed
  - `user`:
    User-wide (global) configuration




## `code-context deinit`

Remove code-context from Claude Code settings

**Usage:** `code-context deinit [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to remove the server configuration from

  Default value: `project`

  Possible values:
  - `project`:
    Project-level configuration (committed to the repo)
  - `local`:
    Local project configuration that is not committed
  - `user`:
    User-wide (global) configuration




## `code-context doctor`

Diagnose code-context configuration and setup

**Usage:** `code-context doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



## `code-context skill`

Deploy code-context skill to agent .skills/ directories

**Usage:** `code-context skill`



## `code-context completion`


Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  code-context completion bash > ~/.local/share/bash-completion/completions/code-context

  # Zsh (add to ~/.zshrc or a file in fpath)
  code-context completion zsh > ~/.zfunc/_code-context

  # Fish
  code-context completion fish > ~/.config/fish/completions/code-context.fish

  # PowerShell
  code-context completion powershell >> $PROFILE


**Usage:** `code-context completion <SHELL>`

###### **Arguments:**

* `<SHELL>` — Shell to generate completion for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




