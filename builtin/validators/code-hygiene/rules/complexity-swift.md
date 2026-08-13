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
      '  - closure_body_length' \
      'cyclomatic_complexity:' '  warning: 15' '  error: 15' \
      '  ignores_case_statements: true' \
      'function_body_length:' '  warning: 250' '  error: 250' \
      'closure_body_length:' '  warning: 250' '  error: 250' > "$work/swiftlint.yml"
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
    if [ -n "$project" ] && grep -qE '^Could not read configuration:' "$work/lint.err"; then
      printf '%s\n' 'complexity-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.' >&2
      lint "" "$@"
    fi
    cat "$work/lint.err" >&2
    if grep -qE '^Could not read contents of `' "$work/lint.err"; then
      printf '%s\n' 'complexity-swift: swiftlint could not read the contents of a file this run names' >&2
      exit 1
    fi
    measured=0
    if [ "$status" -eq 0 ]; then
      measured=1
    elif [ "$status" -eq 2 ] &&
      jq -e 'type == "array" and length > 0' "$work/report.json" >/dev/null 2>&1
    then
      measured=1
    fi
    if [ "$measured" -eq 0 ]; then
      if grep -qE '^Error: No lintable files found at paths:' "$work/lint.err"; then
        exit 0
      fi
      exit 1
    fi
    jq -c '.[] | select(.rule_id == "cyclomatic_complexity"
                        or .rule_id == "function_body_length"
                        or .rule_id == "closure_body_length")
           | {file: .file, line: .line, message: .reason}' "$work/report.json"
  doctor:
    check_command: "which swiftlint jq grep mktemp"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Complexity and Length — Swift

`swiftlint` decides both gates in one run. Three rules carry it:

- `cyclomatic_complexity` — a function with too many decision points.
- `function_body_length` — a function that runs too long.
- `closure_body_length` — a closure that runs too long.

One run answers two prompt rules, so this rule names both in `supersedes`.
`function-length` states "All Function Types: Methods, closures, lambdas,
standalone functions", so its one gate takes two swiftlint rules: one reads a
declaration, and one reads a closure.

Swiftlint's whole metrics group is `cyclomatic_complexity`,
`function_body_length`, `closure_body_length`, `nesting`, `file_length`,
`type_body_length` and `line_length`. It has no cognitive-complexity rule, so
the three named above are the set that answers the two prompt gates.

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
the signature line. `closure_body_length` writes the same words, so the two
rules of this gate count the same lines.

## What each gate reaches, and what neither reaches

Each gate reads a set of declarations. No gate reads a computed variable whose
body holds no closure. Measured with swiftlint 0.65.0, over one body of 300
code lines and one body of cyclomatic complexity 16 in each shape:

| the declaration | `function_body_length` | `cyclomatic_complexity` | `closure_body_length` |
|---|---|---|---|
| `func` | reports | reports | silent |
| `init` | reports | reports | silent |
| `deinit` | reports | silent | silent |
| `subscript` | reports | silent | silent |
| the `get` accessor of a `subscript` | reports | silent | silent |
| a computed `var` | silent | silent | silent |
| the `get` accessor of a computed `var` | silent | silent | silent |
| a `static var` | silent | silent | silent |
| a closure held in a `let` | silent | silent | reports |
| a trailing closure inside a computed `var` | silent | silent | reports |

`function_body_length` names the declaration in its message: `Function body`,
`Initializer body`, `Deinitializer body`, `Subscript body` and `Accessor body`.
`closure_body_length` names `Closure body`, and it anchors the finding on the
opening line of the closure rather than on the declaration that holds it.

The last column is LENGTH alone. `cyclomatic_complexity` reads no closure, and
swiftlint holds no closure complexity rule, so a closure of cyclomatic
complexity 16 in a `let` and the same closure inside a computed `var` each
report nothing. Superseding therefore drops the complexity gate for a closure,
and that is the trade this rule makes.

### Why the closure gate stands at 250

`function-length` states "All Function Types: Methods, closures, lambdas,
standalone functions", so the prompt rule measures a closure and
`function_body_length` does not. `closure_body_length` at the same 250 is what
carries that half of the prompt rule.

swiftlint's own default for that rule is `warning: 30` and `error: 100`, which
is not the 250 the prompt rule states, so the number was measured before it was
taken. The corpus is the one the section "Why `ignores_case_statements` is on"
above uses — Alamofire, swift-nio and vapor at HEAD, 894 `.swift` files, none of
which carries a `.swiftlint.yml` of its own:

| `warning` | findings |
|---|---|
| 20 | 316 |
| 30 | 148 |
| 40 | 76 |
| 50 | 41 |
| 100 | 3 |
| 150 | 2 |
| 200 | 2 |
| 250 | 1 |
| 300 | 0 |

At 250 the whole corpus reports ONE closure:
`NIOCoreBenchmarks/Benchmarks.swift` holds a `let benchmarks: @Sendable () ->
Void` registration block of 259 lines. The other two rules of this gate report 3
findings over the same 894 files, so the closure rule adds 1 finding to 3.

The shape to read is the trailing closure, because a rule that reported every
one of those would make a suppression mandatory on code the prompt rule calls
correct. It does not. Measured with swiftlint 0.65.0 at the gate of 250:

| the shape | `closure_body_length` |
|---|---|
| a SwiftUI `body` of 200 `Text` rows in one `VStack` | silent |
| a SwiftUI `body` of 300 `Text` rows in one `VStack` | reports, 300 lines |
| a SwiftUI `body` of three `VStack` of 100 rows inside a `Group` | reports the outer `Group`, 306 lines |
| a `func testEndToEnd()` holding one `measure { }` of 300 lines | reports, beside `function_body_length` |
| a computed `var` of 300 statement lines, no closure | silent |

Row 3 is the count a nested closure carries: the outer closure counts every
line under it, the inner ones included. Row 4 is one defect reported twice,
because the method and its trailing closure each run past 250; the answer to
both is the same split.

The acceptance test
`the_shipped_swift_complexity_tool_rule_reports_a_long_trailing_closure` holds
rows 1 and 2. The acceptance test
`the_shipped_swift_complexity_tool_rule_reads_no_computed_property_body` holds
row 5, beside one function of 300 body lines that reports.

## Each rule has one gate, and swiftlint then exits 2

The child states `error:` at the same number as `warning:` for each of the three
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
severity, swiftlint exits 2, and stdout carries 1 entry.

## What the script accepts at status 2

The status alone does not tell a measured run from a broken run. A project
`.swiftlint.yml` that states `swiftlint_version:` with a version that is not
installed makes swiftlint write
`warning: Currently running SwiftLint 0.65.0 but configuration specified
version 99.0.0.` to stderr, write 0 bytes to stdout, lint no file, and exit 2.
At status 2 the REPORT tells the two apart: the probe run writes a JSON array,
and the version-mismatch run writes 0 bytes. The report makes that one
distinction. At status 1 the report is 0 bytes for the clean run beside a
project `excluded:` list, and 0 bytes for the run over the directory
`hollow`, which holds no Swift file. The probe file holds one function of
cyclomatic complexity 16. Each status swiftlint 0.65.0 answers with was
measured against the child configuration this script writes:

| what the run is | status | stdout |
|---|---|---|
| a file that holds no function over a gate | 0 | an empty array, 5 bytes |
| the probe file | 2 | 1 entry |
| the probe file beside `swiftlint_version: 99.0.0` | 2 | 0 bytes |
| the probe file beside a project `excluded:` that covers it | 1 | 0 bytes |
| a path that holds no file | 1 | 0 bytes |
| the directory `hollow`, which holds no Swift file | 1 | 0 bytes |

Each row that measured states the number of ENTRIES, and no number of bytes.
The JSON reporter writes the absolute path of the file into each entry, so the
byte count of a report that holds an entry moves with the length of that path.
A later run from a different directory gives a different byte count for the same
run, and a byte count in this table would then read as false. The entry count
does not move with the path, and the entry count is what the script reads. A
report of 0 bytes, and an empty array of 5 bytes, carry no path, so those two
counts do not move.

So the script accepts status 0, and it accepts status 2 only when the report
holds a JSON array of one entry or more. At each other status, and at status 2
with a report of 0 bytes, the script makes one more test, on stderr. Stderr
that holds `Error: No lintable files found at paths:` at the start of a line
exits 0 with no finding, and each other shape exits 1. The section "Each stderr
test reads swiftlint's own message, and not a file name" below states why the
test is anchored. That branch is how the project's `excluded:` list reaches a
clean answer, and the section "A run whose every file the project excludes"
below states it. Measured with a project `.swiftlint.yml` that states
`excluded: [src]`, over one file under `src/` that holds one function of
cyclomatic complexity 16: swiftlint writes `Error: No lintable files found at
paths: 'src/Complex.swift'` to stderr, writes 0 bytes to stdout, and exits 1;
the script reports no finding and exits 0.

The stderr string names the path, and it does not name the reason. Measured
against the child configuration this script writes, 4 shapes each wrote 0
bytes at status 1 with the line `Error: No lintable files found at paths:`: a
project `excluded: [src]` list over `src/Complex.swift`; the directory `hollow`,
which holds no Swift file; the path `src/Absent.swift`, which holds no file;
the file `src/Notes.txt`, whose name does not end in `.swift`. The script
reports 0 findings and exits 0 for 3 of the 4 shapes. The `[ ! -r "$file" ]`
guard runs before swiftlint, and it reports 0 findings and exits 1 with
`complexity-swift cannot read src/Absent.swift` for the path that holds no
file. That guard makes that one distinction, and no test separates the other
3 shapes.

The acceptance test
`the_shipped_swift_complexity_tool_rule_stays_clean_over_a_hollow_directory`
holds the run over that directory to no finding and no tool error.

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
  `only_rules` and every option of each of the three rules, so the rule owns
  what it measures.

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

## Each stderr test reads swiftlint's own message, and not a file name

The script makes three tests on stderr: one for a project configuration
swiftlint cannot read, one for a file swiftlint cannot decode, and one for a run
that found no file to lint. swiftlint writes the PATH of a file into stderr as
well, so a test that reads ALL of stderr answers the file NAME.

swiftlint writes each message of its own at the START of a line, and it writes
the path echo after `Error: `. Each of the three tests is therefore anchored on
the start of a line:

| what the script tests | the line swiftlint writes |
|---|---|
| `^Could not read configuration:` | `Could not read configuration: file Configuration.swift, line 278` |
| ``^Could not read contents of ` `` | ``Could not read contents of `<path>` `` |
| `^Error: No lintable files found at paths:` | `Error: No lintable files found at paths: '<path>'` |

Measured with swiftlint 0.65.0, over one file that holds one function of
cyclomatic complexity 16 under `Generated/`, beside a project `.swiftlint.yml`
that states `excluded: [Generated]`. The file NAME is the one difference between
the rows:

| the file name | the unanchored script | the anchored script |
|---|---|---|
| `Staged.swift` | 0 findings, exit 0 | 0 findings, exit 0 |
| `Could not read contents of.swift` | 0 findings, exit 1, the rule's tool-error line | 0 findings, exit 0 |
| `Could not read configuration.swift` | 1 finding on a file the project excludes | 0 findings, exit 0 |
| `No lintable files found.swift` | 0 findings, exit 0 | 0 findings, exit 0 |

Row 2 broke a run that measured correctly. Row 3 made a WRONG FINDING: the
script dropped the project configuration, ran swiftlint a second time without
it, and reported a file the project excludes. Row 4 moved nothing, because the
decode test stands above that test, and each other broken shape this rule
measured — a version mismatch and a configuration abort — writes no path into
stderr. The anchor holds that test on swiftlint's own message as well.

Each anchored test was measured in both directions. These runs still make a
test fire:

| the run | findings | exit | the rule's own line |
|---|---|---|---|
| the Latin-1 file alone | 0 | 1 | the decode line |
| the Latin-1 file beside one healthy file | 0 | 1 | the decode line |
| a project file that states `child_config: other.yml` | 1 | 0 | the configuration line |
| a project file of bytes that are not YAML | 1 | 0 | the configuration line |
| one file under `Generated/` beside `excluded: [Generated]` | 0 | 0 | none |

These runs make no test fire:

| the run | findings | exit |
|---|---|---|
| one healthy file that holds a finding | 1 | 0 |
| a file the decode words name, on a healthy run | 1 | 0 |
| a file the configuration words name, on a healthy run | 1 | 0 |
| a file that holds `// swiftlint:disable:next cyclomatic_complexity` | 0 | 0 |
| a project configuration that writes `warning: The key(s) 'whitelist_rules' used as rule identifier(s) is/are invalid.` | 1 | 0 |
| the directory `Sources/Hollow.swift`, which holds no Swift file | 0 | 0 |

The acceptance tests
`the_shipped_swift_complexity_tool_rule_measures_a_file_named_for_the_decode_message`
and
`the_shipped_swift_complexity_tool_rule_measures_a_file_named_for_the_configuration_message`
hold rows 2 and 3 of the file-name table to no finding and no tool error.

## A project configuration swiftlint cannot read beside this rule

swiftlint reads the two `--config` paths as one hierarchy, and two shapes of
the project file stop it. Measured over one file that holds one function of
cyclomatic complexity 16:

| the project `.swiftlint.yml` | what swiftlint does |
|---|---|
| `child_config: other.yml` | aborts, exit 134, `There's an ambiguity in the child / parent configuration tree` |
| bytes that are not YAML | aborts, exit 134, `Cannot parse YAML file` |

Each abort writes `Could not read configuration: file Configuration.swift, line
278` to stderr, and leaves stdout empty. The script read that as a broken tool
and exited 1. Both shapes are configurations swiftlint reads on its own, so a
project switched the gate off without meaning to.

The script tests stderr for `Could not read configuration:` at the start of a
line, and it then runs a second time with its own configuration alone. The
section "Each stderr test reads swiftlint's own message, and not a file name"
above states why the test is anchored. The script writes one line to
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

A project configuration can state options for any of the three rules. The
child's block for a rule replaces the parent's block whole, so the project
cannot change the gate this rule measures against.
`swiftlint rules cyclomatic_complexity` names `warning`, `error` and
`ignores_case_statements`, `swiftlint rules function_body_length` names
`warning` and `error`, and `swiftlint rules closure_body_length` names `warning`
and `error`. The child states each of the seven.

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

## A file swiftlint cannot decode

swiftlint reads a source file as UTF-8 and as nothing else. A file that holds
other bytes — a Swift file a person saved in Latin-1, or a binary file under a
`.swift` name — makes swiftlint write ``Could not read contents of `<path>` ``
to stderr. swiftlint then lints no line of that file.

The `[ ! -r "$file" ]` guard admits the file, because the file IS readable. The
DECODE is what fails, and the status does not state it. Measured with swiftlint
0.65.0 over one file that holds `let name = "café"` in Latin-1, above one
function of cyclomatic complexity 16:

| the run | status | stdout | stderr |
|---|---|---|---|
| the Latin-1 file alone | 0 | an empty array, 5 bytes | `Could not read contents of` |
| the Latin-1 file beside one file that holds a finding | 2 | 1 entry | `Could not read contents of` |

Row 1 is the status and the report of a clean file, so the earlier script
reported 0 findings and exited 0, and the engine read a file swiftlint never
read as a clean tree. Row 2 passes the report test of the section above, so the
status and the report tell the two apart in neither row.

The script therefore tests STDERR, before it reads the status. It writes
`complexity-swift: swiftlint could not read the contents of a file this run
names` and exits 1. swiftlint's own message stands above that line, and it names
the path. The acceptance test
`the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_decode`
holds that behaviour.

The test is anchored on the start of a line, because swiftlint writes a path
into stderr as well. Measured with swiftlint 0.65.0 over one file named
`Could not read contents of.swift` under `Generated/`, beside a project
`.swiftlint.yml` that states `excluded: [Generated]`: swiftlint writes
`Error: No lintable files found at paths: 'Generated/Could not read contents
of.swift'`, and a test spelled `grep -qF 'Could not read contents of'` matched
that path echo. The script then wrote its tool-error line and exited 1 over a
run that measured correctly. Measured, ``^Could not read contents of ` ``
matches the decode message and does not match the path echo, and the same run
reports no finding and exits 0. The section "Each stderr test reads swiftlint's
own message, and not a file name" above states each row of that measurement.

## A file that does not parse

swiftlint states no parse failure. It parses with recovery, and it lints the
declarations it recovered. So the script has no signal to read for this shape,
and the rule states the gap rather than a test it cannot make.

Measured with swiftlint 0.65.0, over one file that holds one function of
cyclomatic complexity 16 under a broken head line:

| the head line | findings |
|---|---|
| `@@@` | 1 |
| `(((` | 1 |
| `]]]` | 1 |
| `((( ]]]` | 1 |
| `class {` | 1 |
| `#if` | 1 |
| `this is not swift` | 1 |
| `@@@ this is not swift ((( ]]]` | 0 |

The same measurement over one file whose function body never closes: the run
reports the finding. Over one file that holds `}` above the function, and over
one file that holds `{` under it: the run reports the finding.

The last row of the table is the shape the parser recovers nothing from.
swiftlint writes an empty array to stdout, writes 0 bytes to stderr, and exits
0. That is the answer of a clean file, and no swiftlint flag states the
difference. `swiftc -parse` does state it — measured, it exits 1 for that file
and 0 for the plain one — and this rule does not run it, for two measured
reasons. `swiftc -parse` also exits 1 for the file whose body never closes,
which swiftlint measured correctly, so a `swiftc` gate would trade a true
finding for the shape. And `swiftc` is a Swift toolchain, which
`doctor.check_command` does not name and which a Homebrew swiftlint does not
bring.

## A run answers for the files it is given, and for no other

`swiftlint lint` with no path argument walks the whole tree under the working
directory. A `files`-scope script that hands `"$@"` straight to swiftlint
therefore answers for every Swift file under the repository root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of two files that each hold one function of
cyclomatic complexity 16, with no argument: without the guard the script
reported 2 findings and exited 0; with the guard it reports none and exits 0.
The same script over the two files reports 2. The acceptance test
`the_shipped_swift_complexity_tool_rule_reads_only_the_files_it_is_given` holds
both halves: the run with no argument, and the run over the two files.

## The rule declares no install commands

Homebrew is the supported way to install swiftlint, and it installs the current
version only, so a Homebrew command cannot pin one. Mint can pin one —
`mint install realm/SwiftLint@0.65.0` — but it builds swiftlint from source and
links the result into `~/.mint/bin`, which is not on the path, so the command
cannot make `check_command` pass. The `doctor.fix_hint` states
`brew install swiftlint` instead. `sah doctor` shows that hint as the fix; the
install lifecycle never runs it.

## The annotation an author writes

Selection in the filter is attribution, not exemption. To exempt one
declaration, write `// swiftlint:disable:next <rule>` on the line DIRECTLY
above it, and name the rule the finding names:

    // swiftlint:disable:next function_body_length  one line for each field
    init() {

Measured with swiftlint 0.65.0, over one function that scores 16 against the
gate of 15. Each of these spellings gives no finding:

- `// swiftlint:disable:next cyclomatic_complexity` on the line above the
  `func` line.
- `//swiftlint:disable:next cyclomatic_complexity` with no space after the
  `//`.
- `// swiftlint:disable:next cyclomatic_complexity - flat dispatch` and
  `// swiftlint:disable:next cyclomatic_complexity flat dispatch table`, each
  with a reason after the rule name.
- `// swiftlint:disable:next cyclomatic_complexity function_body_length`, which
  names the two rules of this gate.
- `// swiftlint:disable:next all`.
- `// swiftlint:disable cyclomatic_complexity`, which runs to the end of the
  file or to the matching `// swiftlint:enable`.
- `// swiftlint:disable:this cyclomatic_complexity` on the `func` line itself.
- `// swiftlint:disable:previous cyclomatic_complexity` on the line UNDER the
  `func` line.
- a doc line above the directive.

Each of these spellings gives one finding:

- the directive with a blank line between it and the `func` line.
- the directive with a doc line between it and the `func` line.
- `// SwiftLint:disable:next cyclomatic_complexity` with capital letters.
- `/* swiftlint:disable:next cyclomatic_complexity */` as a block comment.
- `// swiftlint:disable:previous cyclomatic_complexity` on the line ABOVE the
  `func` line, which names the line above the comment.
- `// swiftlint:disable:next function_body_length` above a function the
  COMPLEXITY gate reports, and the same directive with a reason after it. A
  rule name the directive does not hold is not silenced.
- `// swiftlint:disable:next not_a_rule`, and the directive with no rule name
  at all.
- `// noqa: cyclomatic_complexity`.

The directive holds no marker that expires. swiftlint states
`superfluous_disable_command` for that, and `only_rules` in the child
configuration leaves that rule out of every run of this gate. So a directive
stands until an author takes it away.

A `closure_body_length` finding names the line of the closure, so its directive
stands directly above the OPENING line of the closure rather than above the
declaration that holds it. Measured with swiftlint 0.65.0 over one
`let run: () -> Void = { }` of 300 body lines:
`// swiftlint:disable:next closure_body_length  the registration table` above
the `let` line gives no finding, and
`// swiftlint:disable:next function_body_length` in the same place gives one,
because that directive names another rule. Measured over a SwiftUI `body` of
300 `Text` rows in a `VStack`: the directive above the `VStack {` line gives no
finding, and the same directive above the `var body` line gives one.

The first fix a finding asks for is still to split the declaration. The
directive is the second fix, and the text beside it states why.

To exempt a whole directory, add it to the `excluded:` list of the project's
own `.swiftlint.yml`. The section "Generated code" above states that list.

## The carve-outs the two prompt rules state

`cognitive-complexity` exempts a test, generated code and macro expansions, and
a long flat list of simple cases. `function-length` exempts a test, generated
code, a function that is mostly configuration or data, and an initialization
function that sets many fields.

The run reproduces generated code, through the project's own `excluded:` list.
The run reproduces a flat list of `case` arms, through
`ignores_case_statements`. The author answers every other one with the
directive above.

No swiftlint rule of this gate holds an option for any of them. `swiftlint
rules cyclomatic_complexity` names `warning`, `error` and
`ignores_case_statements`. `swiftlint rules function_body_length` and
`swiftlint rules closure_body_length` each name `warning` and `error`. No
option of the three reads a declaration name, a superclass, a file header or a
data line.

### A test, which the run does not drop

Both prompt rules exempt a test, and `cognitive-complexity` names the
DEFINITION as the mark: "Identify a test from its attribute or framework naming
convention at the **definition**, never from the file name. A complex helper
named `build_request` in a file called `foo_test.rs` is still a complex
function and is still listed."

Swift states that convention in XCTest: a test is a method whose name starts
with `test`, in a class that subclasses `XCTestCase`. swiftlint reads neither
mark for these two rules.

Measured with swiftlint 0.65.0 over one file that holds a `func testEndToEnd()`
of 300 body lines inside an `XCTestCase` subclass, beside a `func
buildRequest()` of cyclomatic complexity 16: the run reports both. The
acceptance test
`the_shipped_swift_complexity_tool_rule_reports_a_test_method_and_its_helper`
holds both rows.

The one alternative is the `excluded:` list, which reads the PATH. That reads
the file name, which is the mark the prompt rule forbids, and it silences the
helper beside the test as well. That trades a true finding for the carve-out,
which is the trade `complexity-python` refuses for a test path.

So a complex test method REPORTS, and the author answers it. The first answer
is to move the table walk out of the test. The second answer is
`// swiftlint:disable:next function_body_length` above the method: measured,
the same method then reported nothing.

### Configuration, data and an initializer, which the run does not drop

`function-length` exempts "Functions that are mostly configuration/data (e.g.,
builder patterns with many options)" and "Initialization functions that set
many fields". `function_body_length` counts a data line like a code line.

Measured with swiftlint 0.65.0, at the gate of 250:

| the shape | the message |
|---|---|
| an `init` that sets 260 fields | `Initializer body should span 250 lines or less ... currently spans 260 lines` |
| a builder of 300 `.opt(n)` lines | `Function body ... currently spans 301 lines` |
| a dictionary of 300 entries in a `func` | `Function body ... currently spans 302 lines` |

So a data function and a long initializer REPORT, and the author answers them.
The first answer is to move the data out of the declaration — a `let` table, a
default value for each field, a smaller builder. The second answer is
`// swiftlint:disable:next function_body_length` above the declaration, with
the reason beside it. The acceptance test
`the_shipped_swift_complexity_tool_rule_answers_the_length_gate_annotation`
holds one annotated initializer beside one bare one, and holds the run to
reporting the bare one alone.

### A flat list of simple cases, which the run drops for a `switch` alone

`cognitive-complexity` exempts "Configuration parsing with many options, where
the score comes from a long flat list of simple cases rather than from
nesting". `ignores_case_statements: true` reproduces that carve-out for a
`switch`, and the section "Why `ignores_case_statements` is on" above states
the measurement: 21 of 23 findings over 893 files disappear, and each of the 21
is a flat dispatch table.

A flat `if` chain is the other shape of the same list, and no option drops it.
Measured over one function of 16 flat `if` statements: the run reports it at
the gate of 15. The author answers that one with
`// swiftlint:disable:next cyclomatic_complexity`, or with a `switch`, which
the option then drops. The acceptance test
`the_shipped_swift_complexity_tool_rule_answers_the_complexity_gate_annotation`
holds one annotated function beside one bare one.

### Generated code, which the run drops

The section "Generated code, which the project's own `excluded:` list carves
out" above states this carve-out and its measurement. The project names the
directory its generator writes into, in the `excluded:` list of its own
`.swiftlint.yml`, and the script names that file as the PARENT of its own
configuration.

An author cannot answer this carve-out with the directive. The generator writes
the file again, and the directive goes away each time. That is why the run
makes the test and the author does not.

`cognitive-complexity` also exempts a macro expansion. swiftlint reads the
source text and it makes no build, so it measures the macro use and never the
expansion. No expansion reaches either gate.
