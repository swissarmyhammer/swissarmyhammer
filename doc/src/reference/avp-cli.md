# Command-Line Help for `avp`

This document contains the help content for the `avp` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/avp-cli
```

**Command Overview:**

* [`avp`↴](#avp)
* [`avp init`↴](#avp-init)
* [`avp deinit`↴](#avp-deinit)
* [`avp doctor`↴](#avp-doctor)
* [`avp edit`↴](#avp-edit)
* [`avp new`↴](#avp-new)

## `avp`

AVP - Agent Validator Protocol

Claude Code hook processor that validates tool calls, file changes, and more.

**Usage:** `avp [OPTIONS] [COMMAND]`

###### **Subcommands:**

* `init` — Install AVP hooks into Claude Code settings
* `deinit` — Remove AVP hooks from Claude Code settings and delete .avp directory
* `doctor` — Diagnose AVP configuration and setup
* `edit` — Edit an existing RuleSet in $EDITOR
* `new` — Create a new RuleSet from template

###### **Options:**

* `-d`, `--debug` — Enable debug output to stderr



## `avp init`

Install AVP hooks into Claude Code settings

**Usage:** `avp init [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to install the hooks

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `avp deinit`

Remove AVP hooks from Claude Code settings and delete .avp directory

**Usage:** `avp deinit [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to remove the hooks from

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `avp doctor`

Diagnose AVP configuration and setup

**Usage:** `avp doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



## `avp edit`

Edit an existing RuleSet in $EDITOR

**Usage:** `avp edit [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — RuleSet name (kebab-case)

###### **Options:**

* `--local` [alias: `project`] — Edit in project (.avp/validators/) [default]
* `--global` [alias: `user`] — Edit in user-level directory (~/.avp/validators/)



## `avp new`

Create a new RuleSet from template

**Usage:** `avp new [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — RuleSet name (kebab-case)

###### **Options:**

* `--local` [alias: `project`] — Create in project (.avp/validators/) [default]
* `--global` [alias: `user`] — Create in user-level directory (~/.avp/validators/)



