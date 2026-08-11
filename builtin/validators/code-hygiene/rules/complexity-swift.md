---
name: complexity-swift
description: Swift functions stay under the complexity and length gates — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes:
  - cognitive-complexity
  - function-length
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    for file in "$@"; do
      if [ ! -r "$file" ]; then
        printf 'complexity-swift cannot read %s\n' "$file" >&2
        exit 1
      fi
    done
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' 'only_rules:' '  - cyclomatic_complexity' '  - function_body_length' \
      'cyclomatic_complexity:' '  warning: 15' '  error: 15' \
      '  ignores_case_statements: true' \
      'function_body_length:' '  warning: 250' '  error: 250' > "$work/swiftlint.yml"
    status=0
    lint() {
      parent="$1"
      shift
      status=0
      if [ -n "$parent" ]; then
        swiftlint lint --config "$parent" --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@" \
          > "$work/report.json" 2> "$work/lint.err" || status=$?
      else
        swiftlint lint --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@" \
          > "$work/report.json" 2> "$work/lint.err" || status=$?
      fi
    }
    project=""
    if [ -f .swiftlint.yml ]; then
      project=".swiftlint.yml"
    fi
    lint "$project" "$@"
    if [ -n "$project" ] && grep -qF 'Could not read configuration' "$work/lint.err"; then
      printf '%s\n' 'complexity-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.' >&2
      lint "" "$@"
    fi
    cat "$work/lint.err" >&2
    measured=0
    if [ "$status" -eq 0 ]; then
      measured=1
    elif [ "$status" -eq 2 ] &&
      jq -e 'type == "array" and length > 0' "$work/report.json" >/dev/null 2>&1
    then
      measured=1
    fi
    if [ "$measured" -eq 0 ]; then
      if grep -qF 'No lintable files found' "$work/lint.err"; then
        exit 0
      fi
      exit 1
    fi
    jq -c '.[] | select(.rule_id == "cyclomatic_complexity"
                        or .rule_id == "function_body_length")
           | {file: .file, line: .line, message: .reason}' "$work/report.json"
  doctor:
    check_command: "which swiftlint jq grep mktemp"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Complexity and Length — Swift

`swiftlint` decides both gates in one run. Two rules carry it:

- `cyclomatic_complexity` — a function with too many decision points.
- `function_body_length` — a function that runs too long.

One run answers two prompt rules, so this rule names both in `supersedes`.

Swiftlint's whole metrics group is `cyclomatic_complexity`,
`function_body_length`, `closure_body_length`, `nesting`, `file_length`,
`type_body_length` and `line_length`. It has no cognitive-complexity rule, so
the two named above are the pair that answers the two prompt gates.

## What the complexity gate measures, and what goes with it

`cyclomatic_complexity` counts decision points, with no `+1` base: a probe of
one `for`, one `if` and 14 `else if` scores 16, and the rule reports only when
the count is strictly over the warning level. That is not the published Sonar
cognitive complexity the `complexity` probe computes, so the two numbers need
not agree on the same function.

The threshold stays at 15, the number the `cognitive-complexity` prompt rule
states. The prompt rule's second gate — condition-nesting depth 4 or more — has
its own swiftlint rule, `nesting`, but that rule measures type and function
declarations nested inside each other rather than conditions, so it does not
answer the gate. Superseding therefore drops the nesting gate for Swift. That is
the trade the tool rule makes.

## Why `ignores_case_statements` is on

Without the option, every `case` arm of a `switch` adds one. The prompt rule
says the opposite in as many words: "A `match` or `switch` counts once for the
whole construct. Its arms are branches of one decision", and it carves out "a
long flat list of simple cases".

The difference was measured over 893 `.swift` files — Alamofire, swift-nio and
vapor at HEAD, none of which carries a `.swiftlint.yml` of its own, so the
numbers are not the residue of prior linting:

| `warning` | plain | `ignores_case_statements: true` |
|---|---|---|
| 8 | 118 | 21 |
| 10 | 66 | 12 |
| 12 | 44 | 9 |
| 15 | 23 | 2 |
| 20 | 8 | 1 |

At the gate of 15 the plain form reports 23 findings and 21 of them disappear
when case arms stop counting, because they are flat dispatch tables:
`NIOHTTP1/HTTPEncoder.swift` `write(response:)` scores 121 from 120 one-line
`case` arms each writing a status line, and `NIOPosix/HappyEyeballs.swift`
`processInput` scores 26 from a state-machine `switch`. Under this set's
contract a tool finding is a requirement, so the plain form would make 21
suppressions mandatory on code the prompt rule calls correct.

The option is not free. It drops a `switch` to zero rather than to one, so a
function whose branching lives entirely in nested `switch` statements — the
`while` around a `switch` around a `switch` in `_NIOFileSystem/DirectoryEntries.swift`
— passes the gate. The rule takes that under-count: a missed finding leaves the
review where it was, and a wrong finding is a requirement to change correct code.

## The length gate counts what the prompt rule counts

`function_body_length` reports "excluding comments and whitespace", which is the
`function-length` prompt rule's definition word for word. Measured on a probe of
260 code lines carrying 52 comment-only lines and 52 blank lines, swiftlint
reports 262 — the code lines exactly, because the count covers the body and not
the signature line.

## Each rule has one gate, and swiftlint then exits 2

The child states `error:` at the same number as `warning:` for each of the two
rules, so each rule holds ONE gate. Measured with swiftlint 0.65.0 over a body
of 150 code lines and a body of 300 code lines:

| what the child states | 150-line body | 300-line body |
|---|---|---|
| `warning: 250` and `error: 250` | 0 findings | 1 finding, error severity |
| `warning: 250` alone | 0 findings | 1 finding, warning severity |
| `warning: 250` and `error: 100` | 1 finding, error severity | 1 finding, error severity |

Row 3 is swiftlint's own default error level for `function_body_length`, and it
moves the gate from 250 to 100. Row 1 keeps the count of row 2 at each body,
and it states the option. `error:` with no value is refused: swiftlint answers
`Invalid configuration for 'function_body_length' rule. Falling back to
default.` and measures against `warning: 50`.

swiftlint exits 2 when it reports a finding of error severity, and 0 when it
reports none. Row 1 therefore makes every finding of this rule an exit of 2, so
the script accepts status 2 beside status 0. Measured over one file holding one
function of cyclomatic complexity 16: the run reports 1 finding of error
severity, swiftlint exits 2, and stdout carries 1 entry in 413 bytes.

## What the script accepts at status 2

The status alone does not tell a measured run from a broken run. A project
`.swiftlint.yml` that states `swiftlint_version:` with a version that is not
installed makes swiftlint write
`warning: Currently running SwiftLint 0.65.0 but configuration specified
version 99.0.0.` to stderr, write 0 bytes to stdout, lint no file, and exit 2.
The REPORT tells the two apart. Measured against the child configuration this
script writes, over one file holding one function of cyclomatic complexity 16:

| what the run is | status | stdout |
|---|---|---|
| a file that holds no function over a gate | 0 | an empty array, 5 bytes |
| the probe file | 2 | 1 entry, 413 bytes |
| the probe file beside `swiftlint_version: 99.0.0` | 2 | 0 bytes |
| the probe file beside a project `excluded:` that covers it | 1 | 0 bytes |

So the script accepts status 0, and it accepts status 2 only when the report
holds a JSON array of one entry or more. At each other status, and at status 2
with a report of 0 bytes, the script makes one more test, on stderr. Stderr
that holds `No lintable files found` exits 0 with no finding, and each other
shape exits 1. That branch is how the project's `excluded:` list reaches a
clean answer, and the section "A run whose every file the project excludes"
below states it. Measured with a project `.swiftlint.yml` that states
`excluded: [src]`, over one file under `src/` that holds one function of
cyclomatic complexity 16: swiftlint writes `Error: No lintable files found at
paths: 'src/Complex.swift'` to stderr, writes 0 bytes to stdout, and exits 1;
the script reports no finding and exits 0.

Measured over the same file, beside a project `.swiftlint.yml` that states
`swiftlint_version:`: at `0.65.0` the script reports 1 finding and exits 0; at
`0.64.0`, at `99.0.0` and at `0.1.0` the script reports 0 findings and exits 1,
which the engine reads as a broken tool. A script that accepted every status 2
reported 0 findings and exited 0 for each of those three values, and the engine
read a dirty file as clean. The acceptance test
`the_shipped_swift_complexity_tool_rule_breaks_beside_a_project_version_mismatch`
holds the run beside `swiftlint_version: 99.0.0` to no finding and one tool
error.

## How the run is shaped

The script names TWO configuration files. swiftlint reads a list of `--config`
paths as a parent-child hierarchy. The three shipped swiftlint rules share this
shape, and `missing-docs-swift` states each measurement behind it.

- The PARENT is the project's own `.swiftlint.yml` at the repository root. The
  script names it only when the file is there, because a `--config` path that
  holds no file aborts swiftlint. The parent gives the run the project's
  `excluded:` list.
- The CHILD is the file the script writes into a temporary directory. It states
  `only_rules` and every option of each of the two rules, so the rule owns what
  it measures.

`--force-exclude` makes swiftlint apply the `excluded:` list to a file named as
a command-line argument. `--no-cache` keeps swiftlint from writing a cache
directory into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

## Generated code, which the project's own `excluded:` list carves out

Each prompt rule this rule supersedes carves out generated code.
`cognitive-complexity` exempts "Generated code and macro expansions", and
`function-length` exempts "Generated code". swiftlint holds no generated-code
check of its own: it reads no header line and no file name. A project names the
directory its generator writes into in the `excluded:` list of its own
`.swiftlint.yml`, and that list is the whole carve-out.

Measured over two files that each hold one function of cyclomatic complexity
16, one under `Generated/` and one under `Sources/`, beside a `.swiftlint.yml`
that states `excluded: [Generated]`:

| the run | findings |
|---|---|
| the shipped script | 1 |
| the same script with `--force-exclude` removed | 2 |
| the same script that never names the project configuration | 2 |

The acceptance test
`the_shipped_swift_complexity_tool_rule_reads_the_project_exclude_list` holds
the run to the 1 finding of the first row.

## A run whose every file the project excludes

`--force-exclude` can leave swiftlint no file to read. swiftlint then exits 1
and writes `Error: No lintable files found at paths: ...` to stderr, which reads
as a broken tool. A change that touched generated code alone would then answer
with a tool error rather than with a clean list.

The script tests each file it is given for readability before it starts, so that
message can carry one cause only: the exclude list took every file. Measured
over one file under `Generated/` beside `excluded: [Generated]`: the run reports
no finding, exits 0, and writes swiftlint's own message to stderr. The
acceptance test
`the_shipped_swift_complexity_tool_rule_answers_zero_when_the_project_excludes_every_file`
holds that behaviour.

## A project configuration swiftlint cannot read beside this rule

swiftlint reads the two `--config` paths as one hierarchy, and two shapes of
the project file stop it. Measured over one file that holds one function of
cyclomatic complexity 16:

| the project `.swiftlint.yml` | what swiftlint does |
|---|---|
| `child_config: other.yml` | aborts, exit 134, `There's an ambiguity in the child / parent configuration tree` |
| bytes that are not YAML | aborts, exit 134, `Cannot parse YAML file` |

Each abort writes `Could not read configuration` to stderr, and leaves stdout
empty. The script read that as a broken tool and exited 1. Both shapes are
configurations swiftlint reads on its own, so a project switched the gate off
without meaning to.

The script now tests stderr for `Could not read configuration`, and it then
runs a second time with its own configuration alone. It writes one line to
stderr that names what it dropped. The project's `excluded:` list is not read
for that second run. Measured over one file under `Generated/` that holds the
same function, beside a project file that states `child_config: other.yml` and
`excluded: [Generated]`: the run reports 1 finding, and swiftlint exits 2.

`parent_config:` in the project file is not one of the two shapes. Measured
with `parent_config: other.yml` beside the same file: swiftlint reads both
configurations and reports 1 finding.

The acceptance test
`the_shipped_swift_complexity_tool_rule_measures_beside_a_project_child_config`
holds that behaviour.

## The rule owns its own gates

A project configuration can state options for either rule. The child's block for
a rule replaces the parent's block whole, so the project cannot change the gate
this rule measures against. `swiftlint rules cyclomatic_complexity` names
`warning`, `error` and `ignores_case_statements`, and
`swiftlint rules function_body_length` names `warning` and `error`. The child
states each of the five.

Measured against a project configuration that states `disabled_rules:
[cyclomatic_complexity, function_body_length]` and `cyclomatic_complexity:
warning: 30`, over one file holding one function of cyclomatic complexity 16:
the run reports 1 finding. The same run with both option blocks removed from the
script reports 0. The acceptance test
`the_shipped_swift_complexity_tool_rule_keeps_its_own_gates` holds that pair of
counts.

## A run cannot answer zero for a broken tool

swiftlint exits 1 for a file that is not there, and it writes nothing to stdout.
A shell pipeline takes the exit status of its LAST command, and that command was
`jq`, so the earlier pipe exited 0 and reported nothing. That reads exactly like
a clean file.

The script tests each file it is given before it starts, and it writes
swiftlint's report to a file rather than into a pipe. Measured over one path that
holds no file: the earlier pipe reported no finding and exited 0; the script
reports no finding and exits 1, with `complexity-swift cannot read
Sources/Absent.swift` on stderr. The acceptance test
`the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_read` holds
that behaviour.

`mktemp -d` makes the working directory the script writes the configuration and
the report into, and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers
every way the script leaves: a clean run, a finding, and a failure.

## A run answers for the files it is given, and for no other

`swiftlint lint` with no path argument walks the whole tree under the working
directory. A `files`-scope script that hands `"$@"` straight to swiftlint
therefore answers for every Swift file under the repository root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of two files that each hold one function of
cyclomatic complexity 16, with no argument: without the guard the script
reported 2 findings and exited 0; with the guard it reports none and exits 0.
The acceptance test
`the_shipped_swift_complexity_tool_rule_reads_only_the_files_it_is_given` holds
that behaviour.

## The rule declares no install commands

Homebrew is the supported way to install swiftlint, and it installs the current
version only, so a Homebrew command cannot pin one. Mint can pin one —
`mint install realm/SwiftLint@0.65.0` — but it builds swiftlint from source and
links the result into `~/.mint/bin`, which is not on the path, so the command
cannot make `check_command` pass. The `doctor.fix_hint` states
`brew install swiftlint` instead. `sah doctor` shows that hint as the fix; the
install lifecycle never runs it.

## How to exempt one function

Selection in the filter is attribution, not exemption: to exempt one function,
write `// swiftlint:disable:next cyclomatic_complexity` — or the matching rule
name — above it in the code. To exempt a whole directory, add it to the
`excluded:` list of the project's own `.swiftlint.yml`.
