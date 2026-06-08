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
* [`avp model`↴](#avp-model)
* [`avp model list`↴](#avp-model-list)
* [`avp model show`↴](#avp-model-show)
* [`avp model use`↴](#avp-model-use)
* [`avp completion`↴](#avp-completion)

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
* `model` — Manage AI model configurations
* `completion` — Generate shell completion scripts

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
    Project-level configuration (committed to the repo)
  - `local`:
    Local project configuration that is not committed
  - `user`:
    User-wide (global) configuration




## `avp deinit`

Remove AVP hooks from Claude Code settings and delete .avp directory

**Usage:** `avp deinit [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to remove the hooks from

  Default value: `project`

  Possible values:
  - `project`:
    Project-level configuration (committed to the repo)
  - `local`:
    Local project configuration that is not committed
  - `user`:
    User-wide (global) configuration




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



## `avp model`

Manage AI model configurations

**Usage:** `avp model [COMMAND]`

###### **Subcommands:**

* `list` — List all available models
* `show` — Show the current model configuration
* `use` — Apply a specific model to the project



## `avp model list`

List all available models

**Usage:** `avp model list`



## `avp model show`

Show the current model configuration

**Usage:** `avp model show`



## `avp model use`

Apply a specific model to the project

**Usage:** `avp model use <NAME>`

###### **Arguments:**

* `<NAME>` — Model name to apply



## `avp completion`


Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  avp completion bash > ~/.local/share/bash-completion/completions/avp

  # Zsh (add to ~/.zshrc or a file in fpath)
  avp completion zsh > ~/.zfunc/_avp

  # Fish
  avp completion fish > ~/.config/fish/completions/avp.fish

  # PowerShell
  avp completion powershell >> $PROFILE


**Usage:** `avp completion <SHELL>`

###### **Arguments:**

* `<SHELL>` — Shell to generate completion for

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




