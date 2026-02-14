# Command-Line Help for `avp`

This document contains the help content for the `avp` command-line program.

**Command Overview:**

* [`avp`↴](#avp)
* [`avp init`↴](#avp-init)
* [`avp deinit`↴](#avp-deinit)
* [`avp doctor`↴](#avp-doctor)
* [`avp list`↴](#avp-list)
* [`avp login`↴](#avp-login)
* [`avp logout`↴](#avp-logout)
* [`avp whoami`↴](#avp-whoami)
* [`avp search`↴](#avp-search)
* [`avp info`↴](#avp-info)
* [`avp install`↴](#avp-install)
* [`avp uninstall`↴](#avp-uninstall)
* [`avp new`↴](#avp-new)
* [`avp publish`↴](#avp-publish)
* [`avp unpublish`↴](#avp-unpublish)
* [`avp outdated`↴](#avp-outdated)
* [`avp update`↴](#avp-update)

## `avp`

AVP - Agent Validator Protocol

Claude Code hook processor that validates tool calls, file changes, and more.

**Usage:** `avp [OPTIONS] [COMMAND]`

###### **Subcommands:**

* `init` — Install AVP hooks into Claude Code settings
* `deinit` — Remove AVP hooks from Claude Code settings and delete .avp directory
* `doctor` — Diagnose AVP configuration and setup
* `list` — List all available validators
* `login` — Authenticate with the AVP registry
* `logout` — Log out from the AVP registry
* `whoami` — Show current authenticated user
* `search` — Search the AVP registry for packages
* `info` — Show detailed information about a package
* `install` — Install a package from the registry
* `uninstall` — Remove an installed package
* `new` — Create a new Validator from template
* `publish` — Publish a package to the registry
* `unpublish` — Remove a published package version from the registry
* `outdated` — Check for available package updates
* `update` — Update installed packages to latest versions

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



## `avp list`

List all available validators

**Usage:** `avp list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including descriptions
* `--global` — Show only global (user-level) validators
* `--local` — Show only local (project-level) validators
* `--json` — Output as JSON



## `avp login`

Authenticate with the AVP registry

**Usage:** `avp login`



## `avp logout`

Log out from the AVP registry

**Usage:** `avp logout`



## `avp whoami`

Show current authenticated user

**Usage:** `avp whoami`



## `avp search`

Search the AVP registry for packages

**Usage:** `avp search [OPTIONS] <QUERY>`

###### **Arguments:**

* `<QUERY>` — Search query

###### **Options:**

* `--tag <TAG>` — Filter by tag
* `--json` — Output as JSON



## `avp info`

Show detailed information about a package

**Usage:** `avp info <NAME>`

###### **Arguments:**

* `<NAME>` — Package name



## `avp install`

Install a package from the registry

**Usage:** `avp install [OPTIONS] <PACKAGE>`

###### **Arguments:**

* `<PACKAGE>` — Package name, optionally with @version (e.g. no-secrets@1.2.3)

###### **Options:**

* `--local` [alias: `project`] — Install to project (.avp/validators/) [default]
* `--global` [alias: `user`] — Install globally (~/.avp/validators/)



## `avp uninstall`

Remove an installed package

**Usage:** `avp uninstall [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Package name

###### **Options:**

* `--local` [alias: `project`] — Remove from project (.avp/validators/) [default]
* `--global` [alias: `user`] — Remove from global (~/.avp/validators/)



## `avp new`

Create a new Validator from template

**Usage:** `avp new [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — RuleSet name (kebab-case)

###### **Options:**

* `--local` [alias: `project`] — Create in project (.avp/validators/) [default]
* `--global` [alias: `user`] — Create in user-level directory (~/.avp/validators/)



## `avp publish`

Publish a package to the registry

**Usage:** `avp publish [OPTIONS] [PATH]`

###### **Arguments:**

* `<PATH>` — Path to the RuleSet directory to publish

  Default value: `.`

###### **Options:**

* `--dry-run` — Validate and show what would be published without uploading



## `avp unpublish`

Remove a published package version from the registry

**Usage:** `avp unpublish <NAME_VERSION>`

###### **Arguments:**

* `<NAME_VERSION>` — Package name@version (e.g. no-secrets@1.2.3)



## `avp outdated`

Check for available package updates

**Usage:** `avp outdated`



## `avp update`

Update installed packages to latest versions

**Usage:** `avp update [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Specific package to update (all if omitted)

###### **Options:**

* `--local` [alias: `project`] — Update project packages [default]
* `--global` [alias: `user`] — Update global (~/.avp/validators/) packages



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
