# `.validators/`

The **validator store** for SwissArmyHammer (`sah`). This directory is created
and maintained by `sah init`.

## What's here

Each subdirectory is one **validator set** — a named bundle of code-review rules
(for example `rust/`, `naming/`, `code-security/`). A set is a folder with a
`VALIDATOR.md` (the set manifest) plus a `rules/` directory of rule files. The
review engine reads this directory directly — validators are **not** symlinked
into agent directories.

The set manifest carries the shared metadata, including the `match` block:

    ---
    name: code-hygiene
    description: >-
      Flag hygiene defects in changed source code.
    match:
      files:
        - "@file_groups/source_code"
    probes:
      - callers
    ---

Every rule in the set inherits the set's `match`. A rule file carries `name`
and `description`, then its body.

## Rule kinds

There are two kinds of rule. Both kinds share the same metadata and the same
`match` mechanics. Only execution differs.

- **Prompt rule** — the rule body is a prompt. An LLM reads the body and the
  changed code, and reports the findings. This is the default kind.
- **Tool rule** — the rule frontmatter has a `tool` block. A language tool
  examines the code and reports the findings. No LLM reads the code. The
  result is the same on every run.

A rule with a `tool` block is a tool rule. A rule without one is a prompt
rule. There is no separate directory, no separate schema, and no separate
matcher — a tool rule is a rule with more keys.

## Tool rules

A tool rule binds one tool to one language. Example:

    # rules/missing-docs-python.md
    ---
    name: missing-docs-python
    description: Public items need docs — checked by ruff, not by prompt.
    match:
      files:
        - "**/*.py"
      project_types:
        - python
    supersedes: missing-docs
    tool:
      scope: files
      run: |
        ruff check --select D1 --output-format json "$@" |
          jq -c '.[] | {file: .filename, line: .location.row, message: .message}'
      doctor:
        check_command: "which ruff jq"
        check_version_command: "ruff --version"
      install:
        commands: ["uv tool install ruff", "pipx install ruff", "brew install ruff"]
    ---

The `match` block is the same block the set manifest uses — the same struct,
the same file patterns, the same `@file_groups` references. Two additions:

- A tool rule may carry its own `match`. It narrows the set's match — the rule
  applies to the intersection. A rule never matches a file its set does not
  match.
- `project_types` is a new key in the same `match` block. It names the
  detected project types the rule serves. Install and configuration also key
  on it.

The keys under `match` combine with an implicit AND. Every key that is present
must match. A key that is absent matches everything. The values inside one key
combine with OR. So `files: ["**/*.py"]` plus `project_types: [python]` means:
the file matches the pattern AND the workspace is a python project.

`supersedes` names the prompt rule this tool rule replaces. When the tool is
healthy, the named prompt rule is skipped for the files this rule matches.
When the tool is missing, the named prompt rule runs as before. This is the
fallback mechanism.

The `tool` block keys:

- `run` — a shell script, the same way skills embed shell. Write the pipeline
  you would type in a terminal: the tool, piped through `jq`, `sed`, or
  `grep`. Select the findings this rule owns and shape them in the pipe.
  There is no mapping configuration — the pipe is the mapping.
- `scope` — `files` or `workspace`. With `files`, the script receives the
  changed files as its arguments (`"$@"`). With `workspace`, the script runs
  one time at the workspace root with no arguments (for example `cargo`), and
  the engine keeps only the findings in changed files.
- `doctor` — the commands that show the script's tools are installed and show
  the main tool's version. Name everything the pipe needs (`which ruff jq`).
- `install.commands` — the install commands, in order of preference. Pin the
  tool version in each command. An unpinned tool can change its rules and
  break the gate.

The script's contract is its stdout. One finding per line, in either shape:

- `path:line: message` — the common linter line convention, easy to make
  with `sed` or `grep -n`.
- `{"file": ..., "line": ..., "message": ...}` — a JSON object per line,
  what `jq -c` emits.

Empty stdout means clean. Exit 0 means the script judged the code. A nonzero
exit means the script broke — its stderr goes to the diagnosing agent, and no
findings are read. A pipe that ends in `jq` or `sed` exits 0 even when the
linter before it exits 1 on findings; that is the behavior you want, so do
not add `pipefail` for linters that exit nonzero on findings.

Selection in the pipe is attribution, not exemption. Some tools cannot run
one check alone — `cargo clippy -- -W missing_docs` emits its whole lint set.
The `jq 'select(...)'` or `grep` in your pipe says which findings this rule
owns; the rest are dropped, not reported. To exempt one code item, use an
inline suppression in the code — never the pipe.

Rules for tool rules:

- A finding from a tool is a requirement. Fix it or suppress it in code.
- Exemptions live in the tool configuration or in an inline suppression
  (for example `#[allow(missing_docs)]`, `# noqa`). They do not live in prose.
- When a tool needs a configuration file, the `run` script writes one to a
  temporary path and passes it with a flag — the script owns its whole
  invocation, config included. Never change the project's own lint
  configuration.

### Fixtures

Each tool rule ships two fixture files in the set's `fixtures/` directory:

- `fixtures/<name>.fail.<ext>` — the tool must report at least one finding.
- `fixtures/<name>.pass.<ext>` — the tool must report zero findings.

Doctor runs the tool rule against both fixtures. A tool rule that fails its
fixtures is not used, and doctor reports it. Fixtures catch a tool upgrade
that changes behavior.

## Install lifecycle

When a review needs a tool rule and the tool is missing:

1. Doctor runs `check_command`. If it passes, the tool rule is ready.
2. The engine tries each entry in `install.commands`, in order. After each try,
   doctor runs again.
3. If every command fails, an install agent gets the rule, the platform, and
   the error output. The agent has one goal: make `check_command` pass.
   Doctor confirms the result — the agent cannot assert success.
4. If the tool is still missing, the superseded prompt rule runs instead, and
   doctor keeps a warning. A missing tool degrades the review. It never blocks
   the review.

## Doctor

The review engine is doctorable. `sah doctor` reports, for this project:

- The detected project types.
- Each validator set, and whether it applies to this project.
- Each tool rule for the detected project types: tool present or missing, tool
  version, fixture result.
- Each tool rule on its prompt fallback because its tool is missing, with the
  install commands to fix it.

Run `sah doctor` after install, and again when review behavior changes.

## Customize and override

Validators resolve with this precedence — **later wins**:

    built-in (shipped in sah)  <  user (~/.validators/)  <  this project (./.validators/)

A set or rule in this folder therefore overrides a user-level or built-in one
of the same name, and anything you add here is picked up immediately. Prompt
rules and tool rules override the same way — a rule is a rule.

- **Add your own** — create `./.validators/<set>/VALIDATOR.md` (and `rules/`).
  Validators you add are never touched by `sah init`.
- **Replace a built-in** — give your set or rule the same name as a built-in;
  yours wins by the precedence above.

`sah init` refreshes the built-in validator files on every run but leaves your
own files in place, so keep your changes as your own named set or rule so they
always persist.

## Learn more

Run `sah --help`.
