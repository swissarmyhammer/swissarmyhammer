# Command-Line Help for `shelltool`

This document contains the help content for the `shelltool` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/shelltool-cli
```

**Command Overview:**

* [`shelltool`↴](#shelltool)
* [`shelltool serve`↴](#shelltool-serve)
* [`shelltool init`↴](#shelltool-init)
* [`shelltool deinit`↴](#shelltool-deinit)
* [`shelltool doctor`↴](#shelltool-doctor)
* [`shelltool completion`↴](#shelltool-completion)

## `shelltool`

shelltool - A shell that saves tokens

Replaces Bash and exec CLI tools with a persistent, searchable shell. Instead of flooding the context window with raw command output, shelltool stores everything in history — the agent runs commands, then greps the results, retrieving only the lines that matter.

**Usage:** `shelltool [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `serve` — Run MCP server over stdio, exposing the shell tool
* `init` — Install shelltool MCP server into Claude Code settings
* `deinit` — Remove shelltool from Claude Code settings
* `doctor` — Diagnose shelltool configuration and setup
* `completion` — Generate shell completion scripts

###### **Options:**

* `-d`, `--debug` — Enable debug output to stderr



## `shelltool serve`

Run MCP server over stdio, exposing the shell tool

**Usage:** `shelltool serve`



## `shelltool init`

Install shelltool MCP server into Claude Code settings

**Usage:** `shelltool init [TARGET]`

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




## `shelltool deinit`

Remove shelltool from Claude Code settings

**Usage:** `shelltool deinit [TARGET]`

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




## `shelltool doctor`

Diagnose shelltool configuration and setup

**Usage:** `shelltool doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



## `shelltool completion`


Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  shelltool completion bash > ~/.local/share/bash-completion/completions/shelltool

  # Zsh (add to ~/.zshrc or a file in fpath)
  shelltool completion zsh > ~/.zfunc/_shelltool

  # Fish
  shelltool completion fish > ~/.config/fish/completions/shelltool.fish

  # PowerShell
  shelltool completion powershell >> $PROFILE


**Usage:** `shelltool completion <SHELL>`

###### **Arguments:**

* `<SHELL>` — Shell to generate completion for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




