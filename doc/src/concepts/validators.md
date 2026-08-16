# Validators

Validators are the guardrails of SwissArmyHammer. They are rules-as-data quality gates — focused agents that enforce code quality, security, and test integrity. Each validator is scoped by file globs to the changed files it applies to, and the review pipeline runs the matching validators over those changes.

## What a Validator Is

A validator is an AVP (Agent Validator Protocol) rule set: a collection of rules organized under a `VALIDATOR.md` file. Each rule is a markdown document that describes what to check and how to report violations. The review pipeline processes these rules against the changed files it was given.

## Built-in Validators

SwissArmyHammer ships with a set of built-in validators. Each heading below
names a validator set — one directory of `builtin/validators/` — and each
bullet names a rule that set holds, so you can find the rule file by the name
you read here. The lists are illustrative rather than exhaustive — the
authoritative list is the builtin validator tree itself, which evolves over
time.

### `code-hygiene`

Enforces structural code quality rules:

- `function-length` — catches functions that are too long
- `magic-numbers` — requires named constants for repeated literals and repeated configuration
- `missing-docs` — flags public functions and types with no documentation comment
- `no-commented-code` — flags large blocks of commented-out code
- `dead-code` — flags added symbols with no inbound callers, orphaned modules, and unreachable branches
- `data-driven` — flags a match or if-chain over a known set that is really a table

Four of these — `dead-code`, `function-length`, `magic-numbers` and
`missing-docs` — also ship a per-language **tool rule** beside the prompt
rule. A tool rule runs a language tool instead of an LLM, and it replaces the
prompt rule for the files it matches: `function-length-rust`,
`magic-numbers-python` and `missing-docs-typescript` are three of them. The
set holds one tool rule that replaces no prompt rule — `stuttering-name-go`,
which flags an exported Go name that repeats its package name.
`builtin/validators/README.md` states the whole tool rule contract.

### `code-security`

Catches security vulnerabilities:

- `no-secrets` — flags hardcoded secrets, API keys, and credentials
- `injection` — flags SQL injection, XSS, command injection, and other input validation defects
- `command-safety` — flags destructive shell commands in the diff, such as `rm -rf /`, `DROP TABLE`, and force pushes

### `test-integrity`

Prevents test cheating:

- `no-test-cheating` — flags skipped, disabled, or inappropriately mocked tests
- `no-hard-code` — flags a hard-coded return value written to make a test appear to pass

### `completeness`

Checks that a change reaches every site it must:

- `invariant-propagation` — a localized change to a token, flag, format, or case must reach every site that handles it
- `inverse-operation-coverage` — a change to one direction of a paired operation must exercise the inverse
- `case-sensitivity-coverage` — a change to case-sensitive matching needs one regression test through the changed path
- `public-output-contract` — do not reformat user-facing output, drop an intended side effect, or break a public declaration callers depend on

### `duplication` and `reuse`

- `duplication` — flags verbatim and near-verbatim copied blocks; the set also holds the `rust` and `swift` carve-out rules
- `reuse` — flags reimplementations of existing shared code

### `manifests`

Checks dependency manifests rather than source code, so it runs only when a
manifest changed:

- `unused-dependencies-rust` — flags a dependency a `Cargo.toml` declares that no source file of the package names

### Language sets

`rust`, `python`, `js-ts`, `swift`, `dart`, and `numpy` each hold the rules for
one language or library. A naming or logging rule lives here rather than in
`code-hygiene`, because each one is written for a single language: `js-ts`
holds `naming-and-style`, `swift` holds `naming-clarity`, `casing`, and
`doc-parameter-naming`, and `python` holds `logging`.

## How Validators Work

The review pipeline collects the changed files, matches each validator's `match.files` globs against them, and runs the matching validators over the changes:

```
Changed files
    │
    ├─ Loader matches validators by file glob
    │    ├─ code-hygiene checks the changed source
    │    ├─ code-security checks for secrets
    │    └─ Findings collected with each validator's severity
    │
    └─ Blocking findings (error severity) gate the change
```

Matching is on file globs only — a validator with no `match.files` applies to everything, and one scoped to `*.rs` only runs on Rust changes.

## Setting Up Validators

Built-in validators are always available. Project-specific validators go in `./.validators/` under the workspace root, and user-wide validators in `~/.validators/`.

## Configuring the Review Tool

The review tool reads two optional keys from `.sah/sah.yaml`, both under a `review:` mapping:

| Config key | What it controls | When unset |
|------------|------------------|------------|
| `review.model` | The Claude CLI `--model` switch the review tool runs its validator agents with. | The global default (top-level `model:`) is used; when that is also unset, `haiku`. |
| `review.concurrency` | The number of validator agents run in parallel. Must be a positive integer. | The platform default concurrency is used. |

Claude Code is the only chat executor, so both keys hold a Claude CLI `--model` switch — `haiku`, `sonnet`, `opus`, or a full model id. Set `review.model` in `.sah/sah.yaml` to switch only the review tool; the global default (`model:`) stays untouched. Set the top-level `model:` instead to change the default that every tool — including review — falls back to. A fully unconfigured review scope runs `claude --model haiku`.

A configured `.sah/sah.yaml` looks like:

```yaml
model: sonnet           # global default for all tools
review:
  model: haiku          # review tool overrides the global default
  concurrency: 4        # run 4 validator agents in parallel
```

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
name: my-team-rules
description: The rules this team adds on top of the built-in sets
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
| User `~/.validators/` | User-wide rules |
| Installed via Mirdan | Project or global |

Precedence is builtin → user → project: a project rule set overrides a user rule set of the same name, which overrides the built-in.
