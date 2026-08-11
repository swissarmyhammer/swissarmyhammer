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
      'cyclomatic_complexity:' '  warning: 15' '  ignores_case_statements: true' \
      'function_body_length:' '  warning: 250' > "$work/swiftlint.yml"
    lint() {
      if [ -f .swiftlint.yml ]; then
        swiftlint lint --config .swiftlint.yml --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@"
      else
        swiftlint lint --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@"
      fi
    }
    status=0
    lint "$@" > "$work/report.json" 2> "$work/lint.err" || status=$?
    cat "$work/lint.err" >&2
    if [ "$status" -ne 0 ]; then
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

Naming only `warning:` also disables the rule's default `error:` level. That
matters for `function_body_length`, whose default error level is 100: the 262-line
probe reports as a warning against 250, not as an error against 100, so the gate
is the number this rule writes and nothing else.

## How the run is shaped

The script names TWO configuration files. swiftlint reads a list of `--config`
paths as a parent-child hierarchy. The three shipped swiftlint rules share this
shape, and `missing-docs-swift` states each measurement behind it.

- The PARENT is the project's own `.swiftlint.yml` at the repository root. The
  script names it only when the file is there, because a `--config` path that
  holds no file aborts swiftlint. The parent gives the run the project's
  `excluded:` list.
- The CHILD is the file the script writes into a temporary directory. It states
  `only_rules` and the gate of each of the two rules, so the rule owns what it
  measures.

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

## The rule owns its own gates

A project configuration can state options for either rule. The child's block for
a rule replaces the parent's block whole, so the project cannot change the gate
this rule measures against.

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
