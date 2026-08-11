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

    # rules/complexity-python.md
    ---
    name: complexity-python
    description: Python functions stay under the complexity gate — checked by ruff, not by prompt.
    match:
      files:
        - "**/*.py"
      project_types:
        - python
    supersedes: cognitive-complexity
    tool:
      scope: files
      run: |
        ruff check --isolated --no-cache --config "lint.mccabe.max-complexity=15" --select C901 --output-format json "$@" |
          jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'
      doctor:
        check_command: "which ruff jq"
        check_version_command: "ruff --version"
      install:
        commands: ["uv tool install ruff==0.14.5", "pipx install ruff==0.14.5"]
    ---

That is the frontmatter of `rules/complexity-python.md`, all 22 lines of it,
and its `run` is one pipe. A rule whose tool needs several steps writes a
script rather than one pipe; `rules/missing-docs-python.md` is one of those,
and its script is 51 lines.

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

`supersedes` names the prompt rules this tool rule replaces. When the tool is
healthy, each named prompt rule is skipped for the files this rule matches.
When the tool is missing, each named prompt rule runs as before. This is the
fallback mechanism.

Write one name, or a list of names:

    supersedes: missing-docs

    supersedes:
      - cognitive-complexity
      - function-length

One tool run can replace more than one prompt rule. One `cargo clippy` run
decides cognitive complexity and function length, so that rule names both.

The `tool` block keys:

- `run` — a shell script, the same way skills embed shell. Write the pipeline
  you would type in a terminal: the tool, piped through `jq`, `sed`, or
  `grep`. Select the findings this rule owns and shape them in the pipe.
  There is no mapping configuration — the pipe is the mapping.

  A pipe carries one trap, and it is how a rule answers zero for a broken tool.
  A shell pipeline takes the exit status of its LAST command, so a pipe that
  ends in `jq` exits 0 whatever the tool did. The engine reads exit 0 as "the
  tool judged the code", so a tool that refused to start reports as a clean
  file. Write a pipe only where the tool cannot exit nonzero. Otherwise write a
  script: run the tool into a file, test the status, and exit nonzero yourself.
  `rules/missing-docs-python.md` is the worked example, and its body states each
  status it read. Two more shapes of the same trap:

  - A tool can exit 0 for a file it could not open, and print an empty report.
    Test each file the script is given before the tool starts.
  - A `files`-scope script given NO file must report nothing and exit 0. The
    loop over `"$@"` is a no-op, so a tool handed no path falls back to a
    default target of its own and answers for the whole tree.
  - A script that makes a temporary directory removes it. Write
    `work="$(mktemp -d)"`, then `trap 'rm -rf "$work"' EXIT` under it. The
    trap covers a clean run, a run with findings and a broken run alike, and
    it leaves the exit status of the script alone.
- `scope` — `files` or `workspace`. With `files`, the script receives the
  changed files as its arguments (`"$@"`). With `workspace`, the script runs
  one time at the workspace root with no arguments (for example `cargo`), and
  the engine keeps only the findings in changed files.
- `doctor` — the commands that show the script's tools are installed and show
  the main tool's version. Name everything the script needs (`which ruff jq`).
- `install.commands` — the install commands, in order of preference. Pin the
  tool version in each command. An unpinned tool can change its rules and
  break the gate. A tool that ships with the language toolchain has no package
  to pin (clippy is a `rustup` component), so its rule declares no install
  commands at all. An install command must also put the binary where
  `check_command` can find it, because `check_command` alone decides whether the
  install worked. `uv`, `pipx` and `npm install -g` write a directory a user
  PATH usually holds; a bare `go install` writes `$(go env GOPATH)/bin`, which
  a default PATH does not hold, so the Go rules state
  `GOBIN="$HOME/.local/bin"` and land the binary in the same directory `uv` and
  `pipx` use.
- `doctor.fix_hint` — the command a person runs when there is nothing to
  install. A toolchain component has no package version to pin, so its rule
  states `fix_hint: "rustup component add clippy"` and doctor reports that as
  the fix. A fix hint is text for a person. The install lifecycle never runs
  it, and it never enters `install.commands`.

Every script runs with `SAH_BIN` in its environment, naming the `sah` binary
the engine is running inside. A rule whose tool IS sah — the review engine
calling one of its own ops — writes `"$SAH_BIN"` and never a bare `sah`, so it
can never reach an older copy that happens to sit first on `PATH`. The engine
resolves the value in three steps: an `SAH_BIN` already in the environment,
then `std::env::current_exe()` when its file stem is `sah`, then the bare name.
Such a rule declares no install commands — there is no package to pin, and a
review is already running inside the tool — so its `doctor.fix_hint` names what
a person does when `check_command` still fails.

`sah tool` renders a JSON result as YAML, which this contract cannot read, so
an op a tool rule calls returns PLAIN TEXT: the finding lines and nothing else.

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

The same exit status hides a linter that BROKE. A linter can keep one status
for findings and another status for a failure, and the pipe drops both. So a
pipe is safe only where the tool exits nonzero for findings alone. Where the
tool has a failure status of its own, run it into a file, test the status
against the findings status, and exit nonzero yourself.

One status can carry both a measured run and a broken run. The status of a
failure is then the same as the status of a finding. The script must then test
the REPORT beside the status, and accept the shared status only for the report
shape a measured run writes. Measured with swiftlint 0.65.0: a run that
breaches `warning_threshold:` exits 2 and writes a JSON array that holds one
entry for each finding and one entry more for the threshold — over three
fixtures, 2 entries for magic numbers, 2 for complexity and 3 for missing
docs; a run beside a project `swiftlint_version:` that names a version that
is not installed exits 2, writes 0 bytes and lints no file. The three shipped
swiftlint rules accept status 2 only when the report holds a JSON array of one
entry or more. A script that accepted every status 2 reported 0 findings and
exited 0 for the second shape, and the engine read a dirty file as clean.

A failure status and a clean answer can share a report of 0 bytes. The script
must then test STDERR, and answer clean for the shape stderr names. Measured
with swiftlint 0.65.0: a project `.swiftlint.yml` that holds `excluded: [src]`
makes swiftlint write `Error: No lintable files found at paths:
'src/Magic.swift'` to stderr, write 0 bytes to stdout, and exit 1. Each of the
three shipped swiftlint rules tests stderr for `No lintable files found` after
the status gate, and each exits 0 with no finding for that shape. Measured over
three dirty fixtures beside that project file: each of the three reported 0
findings at exit 0. A script without the stderr test answers a tool error for
each project `excluded:` list.

Selection in the pipe is attribution, not exemption. Some tools cannot run
one check alone — `cargo clippy -- -W missing_docs` emits its whole lint set.
The `jq 'select(...)'` or `grep` in your pipe says which findings this rule
owns; the rest are dropped, not reported. To exempt one code item, use an
inline suppression in the code — never the pipe.

Rules for tool rules:

- A finding from a tool is a requirement. Fix it or suppress it in code.
- Exemptions live in the tool configuration or in an inline suppression
  (for example `#[allow(missing_docs)]`, `# noqa`). They do not live in prose.
- **An exemption a person would argue for in prose must become an inline
  suppression the tool reads.** A prompt rule can carve out a case by describing
  it; a tool rule cannot, and it must not try. Turn the argument into a marker in
  the code, and state the marker in the rule body.

  Staged work is the canonical example. A prompt rule can say "exempt a symbol a
  later task will consume", and then every reader decides for themselves what
  counts. A tool rule says instead: write
  `#[expect(dead_code, reason = "...")]`, or `//lint:ignore U1000 <reason>`, or
  `// ts-prune-ignore-next`, or `# noqa: V103`, or `// ignore: unused_element`,
  or `// periphery:ignore`. The marker is the claim, the reason names the change
  that lands the consumer, and a symbol with neither is dead. That converts a
  judgment into a fact, which is the whole point of a tool rule.

  Prefer a marker that expires. Rust's `#[expect]` raises
  `unfulfilled_lint_expectations` the moment the consumer lands, so the
  annotation cleans itself up; `#[allow]` never does. Where the language offers
  both, the rule body names the expiring one.
- When a tool needs a configuration file, the `run` script writes one to a
  temporary path and passes it with a flag — the script owns its whole
  invocation, config included. Never change the project's own lint
  configuration.
- A tool with no configuration flag reads its configuration from the directory
  tree around the file it examines. The script then builds that tree: it makes
  a temporary package, writes the configuration into it, copies the changed
  files in, runs the tool on the package, and maps the temporary paths back to
  the paths it was given. `dart analyze` works this way. The script writes the
  configuration of that tree itself, and it copies no configuration of the
  project's own into the tree.
- A script MAY read the project's own configuration for the FILE LIST alone,
  and only where the tool merges two configurations and lets the script's own
  one win. Which files a linter passes over — a generated tree, a vendored
  tree — is the project's decision and belongs in the project's file. What the
  rule MEASURES is the rule's decision. The three shipped swiftlint rules do
  this: each names the project's `.swiftlint.yml` as the PARENT config and its
  own temporary file as the CHILD, and passes `--force-exclude` so the
  project's `excluded:` list reaches a file named on the command line.

  A script that reads the project's configuration must state EVERY option of
  EVERY rule it measures with, in its own child configuration, and the rule
  body must carry a measurement of a project configuration that states other
  options. Without that, a project silently changes the gate. Measured with
  swiftlint 0.65.0: a parent stating `missing_docs: excludes_inherited_types:
  false` moves the count when the child states no `missing_docs:` block, and it
  moves nothing when the child states the block.

### Fixtures

Each tool rule ships two fixture files in the set's `fixtures/` directory:

- `fixtures/<name>.fail.<ext>.tmpl` — the tool must report at least one finding.
- `fixtures/<name>.pass.<ext>.tmpl` — the tool must report zero findings.

**A fixture is a template, and its stored name ends in `.tmpl`.** A fixture
carries the very defect its rule reports. Stored under a real source extension
it is a file the review engine reviews, so the missing-docs rule fires on the
fixture built to make it fire, and the two demands cannot both be met. No
language owns `.tmpl` and no file group matches it, so the stored file is
never reviewed and never linted. Every file in `fixtures/` takes the suffix,
support files included.

The two fixtures must cover the same kinds. The fail fixture holds one
undocumented item of every kind the pass fixture documents — the type, the
class, the interface, the enumeration, the method, and the function. A pass
fixture that documents six kinds against a fail fixture that holds only a
function proves nothing about the other five: a tool that stops reporting a
whole kind still passes that pair.

Doctor copies `fixtures/` into a scratch directory, drops the `.tmpl` suffix
from every name, and runs there — so the tool sees `missing-docs-rust.fail.rs`
even though the set stores `missing-docs-rust.fail.rs.tmpl`. The set's own
directory is never the working directory, so a tool that writes beside its
input cannot dirty the repository. A `files`-scope script gets the
materialized fixture file name as its argument; a `workspace`-scope script
gets none. Doctor counts only the
findings the run reports ABOUT the fixture under test — the same attribution
the engine makes when it keeps only the findings in the changed files. A
`workspace`-scope script reads the whole `fixtures/` directory on both runs, so
without that attribution it could never pass the pair.

A tool that needs more than a loose file to run gets it in `fixtures/`. Cargo
lints a package, never a loose file, so the `code-hygiene` fixtures carry a
`Cargo.toml.tmpl` and a crate root that hold every Rust fixture as a module. A
`go.mod.tmpl`, a `tsconfig.json.tmpl` and a `Package.swift.tmpl` do the same for
staticcheck, ts-prune and periphery. The whole directory is materialized, not
only the fixture under test, so a `workspace`-scope tool finds those neighbours.

Only the top level of `fixtures/` is copied, so a manifest has to reach its
fixtures without a source directory: the Swift manifest names `path: "."` and
lists its `sources`, and the crate root uses `#[path = "..."]`. Each manifest
names its own rule's fixtures and no others, so one rule's fixtures cannot move
another rule's result.

A tool rule that fails its fixtures is not used, and doctor reports it.
Fixtures catch a tool upgrade that changes behavior.

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

`sah init` runs steps 1 and 2 for every tool rule of the detected project
types, so the tools are already there the first time a review needs them. It
never runs step 3 — install never spends an agent turn. A tool `sah init`
could not install is a warning that names the rule, never an error.

One install runs at a time. An exclusive lock covers steps 2 and 3 together,
so two installers never write one destination at once. A process that waits out
the lock installs nothing and reports the tool blocked, which lands on step 4:
the superseded prompt rule runs. The lock covers the processes that share one
temporary directory, which on a machine that gives each user a temporary
directory of its own means one user. Homebrew locks its own shared prefix.
`npm install -g` under a Homebrew node writes that shared prefix and takes
neither lock; two users installing at that moment are not serialized.

## Doctor

The review engine is doctorable. `sah doctor` reports, for this project:

- The detected project types.
- Each validator set, and whether it applies to this project.
- Each tool rule for the detected project types: tool present or missing, tool
  version, fixture result.
- Each tool rule on its prompt fallback because its tool is missing, with the
  install commands to fix it, or with the `doctor.fix_hint` when the rule
  declares no install commands.

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
