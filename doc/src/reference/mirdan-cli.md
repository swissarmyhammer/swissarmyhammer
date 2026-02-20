# Command-Line Help for `mirdan`

This document contains the help content for the `mirdan` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/mirdan-cli
```

**Command Overview:**

* [`mirdan`↴](#mirdan)
* [`mirdan agents`↴](#mirdan-agents)
* [`mirdan new`↴](#mirdan-new)
* [`mirdan new skill`↴](#mirdan-new-skill)
* [`mirdan new validator`↴](#mirdan-new-validator)
* [`mirdan install`↴](#mirdan-install)
* [`mirdan uninstall`↴](#mirdan-uninstall)
* [`mirdan list`↴](#mirdan-list)
* [`mirdan search`↴](#mirdan-search)
* [`mirdan info`↴](#mirdan-info)
* [`mirdan login`↴](#mirdan-login)
* [`mirdan logout`↴](#mirdan-logout)
* [`mirdan whoami`↴](#mirdan-whoami)
* [`mirdan publish`↴](#mirdan-publish)
* [`mirdan unpublish`↴](#mirdan-unpublish)
* [`mirdan outdated`↴](#mirdan-outdated)
* [`mirdan update`↴](#mirdan-update)
* [`mirdan sync`↴](#mirdan-sync)
* [`mirdan doctor`↴](#mirdan-doctor)

## `mirdan`

Mirdan manages skills (agentskills.io spec) and validators (AVP spec) across all detected AI coding agents.

Skills are deployed to each agent's skill directory (e.g. .claude/skills/, .cursor/skills/).
Validators are deployed to .avp/validators/ (project) or ~/.avp/validators/ (global).

Environment variables:
  MIRDAN_REGISTRY_URL  Override the registry URL (useful for local testing)
  MIRDAN_TOKEN         Provide an auth token without logging in
  MIRDAN_CREDENTIALS_PATH  Override the credentials file location
  MIRDAN_AGENTS_CONFIG     Override the agents configuration file

**Usage:** `mirdan [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `agents` — Detect and list installed AI coding agents
* `new` — Create a new skill or validator from template
* `install` — Install a skill, validator, or MCP server (type auto-detected from contents)
* `uninstall` — Remove an installed skill or validator package
* `list` — List installed skills and validators
* `search` — Search the registry for skills and validators
* `info` — Show detailed information about a package
* `login` — Authenticate with the registry
* `logout` — Log out from the registry and revoke token
* `whoami` — Show current authenticated user
* `publish` — Publish a skill or validator to the registry (type auto-detected)
* `unpublish` — Remove a published package version from the registry
* `outdated` — Check for available package updates
* `update` — Update installed packages to latest versions
* `sync` — Reconcile .skills/ with agent directories and verify lockfile
* `doctor` — Diagnose Mirdan setup and configuration

###### **Options:**

* `-d`, `--debug` — Enable debug output to stderr
* `-y`, `--yes` — Skip confirmation prompts (useful for CI/CD)
* `--agent <AGENT_ID>` — Limit operations to a single agent (e.g. claude-code, cursor)



## `mirdan agents`

Detect and list installed AI coding agents

**Usage:** `mirdan agents [OPTIONS]`

###### **Options:**

* `--all` — Show all known agents, not just detected ones
* `--json` — Output as JSON



## `mirdan new`

Create a new skill or validator from template

**Usage:** `mirdan new <COMMAND>`

###### **Subcommands:**

* `skill` — Scaffold a new skill (agentskills.io spec)
* `validator` — Scaffold a new validator (AVP spec)



## `mirdan new skill`

Scaffold a new skill (agentskills.io spec)

**Usage:** `mirdan new skill [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Skill name (kebab-case, 1-64 chars)

###### **Options:**

* `--global` — Create in agent global skill directories instead of project-level



## `mirdan new validator`

Scaffold a new validator (AVP spec)

**Usage:** `mirdan new validator [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Validator name (kebab-case, 1-64 chars)

###### **Options:**

* `--global` — Create in ~/.avp/validators/ instead of .avp/validators/



## `mirdan install`

Install a skill, validator, or MCP server (type auto-detected from contents)

**Usage:** `mirdan install [OPTIONS] <PACKAGE>`

###### **Arguments:**

* `<PACKAGE>` — Package name, name@version, ./local-path, owner/repo, or git URL

###### **Options:**

* `--global` — Install globally (~/.avp/validators/ for validators, agent global dirs for skills)
* `--git` — Treat package as a git URL (clone instead of registry lookup)
* `--skill <SKILL>` — Install a specific skill/validator by name from a multi-package repo
* `--mcp` — Install as an MCP server instead of a skill/validator
* `--command <COMMAND>` — MCP server command (binary to run). Required when --mcp is set
* `--args <ARGS>` — MCP server arguments



## `mirdan uninstall`

Remove an installed skill or validator package

**Usage:** `mirdan uninstall [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Package name

###### **Options:**

* `--global` — Remove from global locations



## `mirdan list`

List installed skills and validators

**Usage:** `mirdan list [OPTIONS]`

###### **Options:**

* `--skills` — Show only skills
* `--validators` — Show only validators
* `--json` — Output as JSON



## `mirdan search`

Search the registry for skills and validators

With a query argument, performs a single search and prints results. Without a query, enters interactive fuzzy search mode.

**Usage:** `mirdan search [OPTIONS] [QUERY]`

###### **Arguments:**

* `<QUERY>` — Search query (omit for interactive mode)

###### **Options:**

* `--json` — Output as JSON



## `mirdan info`

Show detailed information about a package

**Usage:** `mirdan info <NAME>`

###### **Arguments:**

* `<NAME>` — Package name



## `mirdan login`

Authenticate with the registry

Opens a browser for OAuth login. The registry URL can be overridden with MIRDAN_REGISTRY_URL for local testing.

**Usage:** `mirdan login`



## `mirdan logout`

Log out from the registry and revoke token

**Usage:** `mirdan logout`



## `mirdan whoami`

Show current authenticated user

**Usage:** `mirdan whoami`



## `mirdan publish`

Publish a skill or validator to the registry (type auto-detected)

Auto-detects package type from directory contents: - SKILL.md present -> publishes as a skill - VALIDATOR.md + rules/ present -> publishes as a validator

**Usage:** `mirdan publish [OPTIONS] [SOURCE]`

###### **Arguments:**

* `<SOURCE>` — Path or git URL to the package directory to publish

  Default value: `.`

###### **Options:**

* `--dry-run` — Validate and show what would be published without uploading



## `mirdan unpublish`

Remove a published package version from the registry

**Usage:** `mirdan unpublish <NAME_VERSION>`

###### **Arguments:**

* `<NAME_VERSION>` — Package name@version (e.g. my-skill@1.0.0)



## `mirdan outdated`

Check for available package updates

**Usage:** `mirdan outdated`



## `mirdan update`

Update installed packages to latest versions

**Usage:** `mirdan update [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Specific package to update (all if omitted)

###### **Options:**

* `--global` — Update global packages



## `mirdan sync`

Reconcile .skills/ with agent directories and verify lockfile

**Usage:** `mirdan sync [OPTIONS]`

###### **Options:**

* `--global` — Sync global locations



## `mirdan doctor`

Diagnose Mirdan setup and configuration

**Usage:** `mirdan doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



