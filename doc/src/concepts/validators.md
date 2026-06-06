# Validators

Validators are the guardrails of SwissArmyHammer. They are rules-as-data quality gates — focused agents that enforce code quality, security, and test integrity. Each validator is scoped by file globs to the changed files it applies to, and the review pipeline runs the matching validators over those changes.

## What a Validator Is

A validator is an AVP (Agent Validator Protocol) rule set: a collection of rules organized under a `VALIDATOR.md` file. Each rule is a markdown document that describes what to check and how to report violations. AVP processes these rules against the agent's tool calls and file changes in real time.

## Built-in Validators

SwissArmyHammer ships with four validator sets:

### Code Quality

Enforces structural code quality rules:

- **Cognitive complexity** — flags overly complex functions
- **Function length** — catches functions that are too long
- **Naming consistency** — enforces naming conventions
- **No commented-out code** — prevents dead code from accumulating
- **No hard-coded values** — catches embedded credentials and config
- **No magic numbers** — requires named constants
- **No string equality** — flags fragile string comparisons
- **No log truncation** — ensures complete error logging
- **Missing docs** — flags undocumented public APIs

### Security Rules

Catches security vulnerabilities:

- **Input validation** — ensures user input is validated at boundaries
- **No secrets** — prevents credentials, tokens, and keys from being committed

### Command Safety

Guards against destructive shell commands:

- **Safe commands** — blocks `rm -rf /`, `DROP TABLE`, force pushes, and other dangerous operations

### Test Integrity

Prevents test cheating:

- **No test cheating** — catches mocking of the thing under test, assertion-free tests, and other patterns that make tests pass without testing anything

## How Validators Work

The review pipeline collects the changed files, matches each validator's `match.files` globs against them, and runs the matching validators over the changes:

```
Changed files
    │
    ├─ Loader matches validators by file glob
    │    ├─ Code quality validator checks the changed source
    │    ├─ Security validator checks for secrets
    │    └─ Findings collected with each validator's severity
    │
    └─ Blocking findings (error severity) gate the change
```

Matching is on file globs only — a validator with no `match.files` applies to everything, and one scoped to `*.rs` only runs on Rust changes.

## Setting Up Validators

Built-in validators are always available. Project-specific validators go in `./.validators/`, and user-wide validators in `$XDG_DATA_HOME/validators/` (default `~/.local/share/validators/`).

## Creating Custom Validators

A validator rule set is a directory with a `VALIDATOR.md` and a `rules/` directory. Each rule is a markdown file describing what to check.

The `VALIDATOR.md` frontmatter declares:

- `name` — the rule set identifier (defaults to the directory name).
- `description` — what the rule set checks.
- `match.files` — file glob patterns that scope the rule set to the changed files it applies to. Supports `@file_groups/...` includes (e.g. `@file_groups/source_code`) that expand to shared pattern lists. Matching is on file globs only.
- `severity` — default severity for the rules (`info`, `warn`, or `error`).
- `tags` — optional labels for filtering and organization.
- `probes` — optional list of probe names (plain strings) the rule set requests from the probe catalog.

```yaml
---
name: dead-code
description: Flags symbols with no inbound callers
match:
  files:
    - "@file_groups/source_code"
severity: error
probes:
  - callers
---
```

The legacy `trigger` key (which named a Claude Code hook event) has been removed. The loader is lenient — a leftover `trigger` still loads — but `check validators` flags it so you can remove it.

## Sharing Validators

Validators can be published and installed via Mirdan:

```bash
# Create and publish
mirdan new validator my-team-rules
mirdan publish

# Install on another project
mirdan install my-team-rules
```

This lets teams codify their standards as installable packages — new projects get the team's quality rules with a single command.

## Validator Locations

| Location | Scope |
|----------|-------|
| Built-in (embedded in binary) | Always available |
| Project `./.validators/` | Project-specific rules |
| Global `$XDG_DATA_HOME/validators/` (default `~/.local/share/validators/`) | User-wide rules |
| Installed via Mirdan | Project or global |

Precedence is builtin → user → project: a project rule set overrides a user rule set of the same name, which overrides the built-in.
