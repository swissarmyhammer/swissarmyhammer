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

## `shelltool`

shelltool - Standalone MCP shell tool for AI coding agents

Serves the SwissArmyHammer shell tool over MCP stdio, giving AI agents a persistent virtual shell with history, process management, and semantic search.

**Usage:** `shelltool [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `serve` — Run MCP server over stdio, exposing the shell tool
* `init` — Install shelltool MCP server into Claude Code settings
* `deinit` — Remove shelltool from Claude Code settings
* `doctor` — Diagnose shelltool configuration and setup

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
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `shelltool deinit`

Remove shelltool from Claude Code settings

**Usage:** `shelltool deinit [TARGET]`

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




## `shelltool doctor`

Diagnose shelltool configuration and setup

**Usage:** `shelltool doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



