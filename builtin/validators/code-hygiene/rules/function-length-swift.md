---
name: function-length-swift
description: Swift declarations and closures stay under the length gate — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: function-length
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    for file in "$@"; do
      if [ ! -e "$file" ]; then
        printf 'sah-diagnostic: function-length-swift found no file at %s, so its bodies are unread\n' "$file" >&2
      fi
    done
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' 'only_rules:' '  - function_body_length' \
      '  - closure_body_length' \
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
      printf '%s\n' 'sah-diagnostic: function-length-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.' >&2
      lint "" "$@"
    fi
    cat "$work/lint.err" >&2
    sed -n 's/^Could not read contents of `\(.*\)`$/sah-diagnostic: swiftlint could not read the contents of \1, so its bodies are unread/p' "$work/lint.err" >&2
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
        for file in "$@"; do
          if [ -e "$file" ]; then
            printf 'sah-diagnostic: function-length-swift judged no file at %s, so its bodies are unread\n' "$file" >&2
          fi
        done
        exit 0
      fi
      exit 1
    fi
    jq -c '.[] | select(.rule_id == "function_body_length"
                        or .rule_id == "closure_body_length")
           | {file: .file, line: .line, message: .reason}' "$work/report.json"
  doctor:
    check_command: "which swiftlint jq grep sed cat mktemp"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Function Length — Swift

`swiftlint` decides the gate in one run. Two rules carry it:

- `function_body_length` — a declaration that runs too long.
- `closure_body_length` — a closure that runs too long.

`function-length` states "All Function Types: Methods, closures, lambdas,
standalone functions", so its one gate takes two swiftlint rules: one reads a
declaration, and one reads a closure.

Every measurement below was made with swiftlint 0.65.0.

## The metric IS the prompt rule's own count

`function_body_length` reports "excluding comments and whitespace", which is the
`function-length` prompt rule's definition word for word. Measured on a probe of
262 code lines carrying 53 comment-only lines and 53 blank lines, swiftlint
reports 262 — the code lines exactly, because the count covers the body and not
the signature line. `closure_body_length` writes the same words, so the two
rules of this gate count the same lines.

So the gate carries the prompt rule's own number, 250, with no derivation. That
is the shape `function-length-dart` and `function-length-rust` each take, and it
is why neither of the three derives a ratio the way `function-length-go` and
`function-length-python` do — those two count STATEMENTS rather than lines.

## The corpus the gate was measured over

Three Swift repositories, cloned at HEAD on 2026-08-15, none of which carries a
`.swiftlint.yml` of its own, so the numbers are not the residue of prior
linting:

| repository | commit | `.swift` files |
|---|---|---|
| Alamofire/Alamofire | `0455bfb650893e86ad07ace16e5f2d36dadf46f4` | 98 |
| apple/swift-nio | `48119dbbd23e3eabba48952ac7f75ebeeb87c217` | 554 |
| vapor/vapor | `c6818be25fa64ccaf3dd2e0be184d96ab4c322a0` | 242 |

894 files. The corpus was run one time with both rules at `warning: 1` and
`error: 1`, which makes swiftlint report every body and print that body's own
line count in its message — `currently spans 259 lines`. 16790 bodies came back
with their own number, 9807 of them declarations and 6983 closures, so every
sweep below is arithmetic on the tool's own count rather than on a model of it.

| `warning` and `error` | findings | `function_body_length` | `closure_body_length` |
|---|---|---|---|
| 100 | 42 | 39 | 3 |
| 150 | 10 | 8 | 2 |
| 200 | 6 | 4 | 2 |
| 250 | 2 | 1 | 1 |
| 300 | 0 | 0 | 0 |

At the gate of 250 the whole corpus reports two bodies:

- `NIOHTTP1/HTTPEncoder.swift` `write(response:)`, 251 lines.
- `NIOCoreBenchmarks/Benchmarks.swift`, a
  `let benchmarks: @Sendable () -> Void` registration block of 259 lines.

The closure rule therefore adds 1 finding to 1 over 894 files, so each of the
two rules of this gate carries half of what the corpus reports.

## Why the closure gate stands at 250 as well

swiftlint's own default for `closure_body_length` is `warning: 30` and
`error: 100`, which is not the 250 the prompt rule states, so the number was
measured before it was taken. Over the same 894 files:

| `warning` | findings |
|---|---|
| 20 | 319 |
| 30 | 148 |
| 40 | 76 |
| 50 | 41 |
| 100 | 3 |
| 150 | 2 |
| 200 | 2 |
| 250 | 1 |
| 300 | 0 |

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

Row 3 is the count a nested closure carries: the outer closure counts every line
under it, the inner ones included. Row 4 is one defect reported twice, because
the method and its trailing closure each run past 250; the answer to both is the
same split.

## What each gate reaches, and what neither reaches

Each rule reads a set of declarations. No rule reads a computed variable whose
body holds no closure. Measured with swiftlint 0.65.0, over one body of 300 code
lines in each shape:

| the declaration | `function_body_length` | `closure_body_length` |
|---|---|---|
| `func` | reports | silent |
| `init` | reports | silent |
| `deinit` | reports | silent |
| `subscript` | reports | silent |
| the `get` accessor of a `subscript` | reports | silent |
| a computed `var` | silent | silent |
| the `get` accessor of a computed `var` | silent | silent |
| a `static var` | silent | silent |
| a closure held in a `let` | silent | reports |
| a trailing closure inside a computed `var` | silent | reports |

`function_body_length` names the declaration in its message: `Function body`,
`Initializer body`, `Deinitializer body`, `Subscript body` and `Accessor body`.
`closure_body_length` names `Closure body`, and it anchors the finding on the
opening line of the closure rather than on the declaration that holds it.

The computed variable is the one gap, and the closure rule closes most of it: a
computed `var` whose body IS a closure reports, and only a computed `var` of
plain statement lines escapes.

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
moves the gate from 250 to 100. Row 1 keeps the count of row 2 at each body, and
it states the option. `error:` with no value is refused: swiftlint answers
`Invalid configuration for 'function_body_length' rule. Falling back to
default.` and measures against `warning: 50`.

swiftlint exits 2 when it reports a finding of error severity, and 0 when it
reports none. Row 1 therefore makes every finding of this rule an exit of 2, so
the script accepts status 2 beside status 0.

## What the script accepts at status 2

The status alone does not tell a measured run from a broken run. A project
`.swiftlint.yml` that states `swiftlint_version:` with a version that is not
installed makes swiftlint write
`warning: Currently running SwiftLint 0.65.0 but configuration specified
version 99.0.0.` to stderr, write 0 bytes to stdout, lint no file, and exit 2.
At status 2 the REPORT tells the two apart: the probe run writes a JSON array,
and the version-mismatch run writes 0 bytes. The report makes that one
distinction. At status 1 the report is 0 bytes for the clean run beside a
project `excluded:` list, and 0 bytes for the run over a directory that holds no
Swift file. The probe file holds one function of 302 body lines. Each status
swiftlint 0.65.0 answers with was measured against the child configuration this
script writes:

| what the run is | status | stdout |
|---|---|---|
| a file that holds no body over the gate | 0 | an empty array, 5 bytes |
| the probe file | 2 | 1 entry |
| the probe file beside `swiftlint_version: 99.0.0` | 2 | 0 bytes |
| the probe file beside a project `excluded:` that covers it | 1 | 0 bytes |
| a path that holds no file | 1 | 0 bytes |
| a directory that holds no Swift file | 1 | 0 bytes |

Each row that measured states the number of ENTRIES, and no number of bytes. The
JSON reporter writes the absolute path of the file into each entry, so the byte
count of a report that holds an entry moves with the length of that path. A
later run from a different directory gives a different byte count for the same
run, and a byte count in this table would then read as false. The entry count
does not move with the path, and the entry count is what the script reads. A
report of 0 bytes, and an empty array of 5 bytes, carry no path, so those two
counts do not move.

So the script accepts status 0, and it accepts status 2 only when the report
holds a JSON array of one entry or more. At each other status, and at status 2
with a report of 0 bytes, the script makes one more test, on stderr. Stderr that
holds `Error: No lintable files found at paths:` at the start of a line exits 0
with no finding, and each other shape exits 1. The section "Each stderr reading
takes swiftlint's own message, and not a file name" below states why the test is
anchored. That branch is how the project's `excluded:` list reaches a clean
answer, and the section "A run whose every file the project excludes" below
states it.

Measured over the probe file beside a project `.swiftlint.yml` that states
`swiftlint_version:`: at `0.65.0` the script reports 1 finding and exits 0; at
`0.64.0`, at `99.0.0` and at `0.1.0` the script reports 0 findings and exits 1,
which the engine reads as a broken tool. A script that accepted every status 2
reported 0 findings and exited 0 for each of those three values, and the engine
read a dirty file as clean.

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

`mktemp -d` makes the working directory the script writes the configuration and
the report into, and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers
every way the script leaves: a clean run, a finding, and a failure.

## The rule owns its own gates

A project configuration can state options for either of the two rules. The
child's block for a rule replaces the parent's block whole, so the project
cannot change the gate this rule measures against.
`swiftlint rules function_body_length` names `warning` and `error`, and
`swiftlint rules closure_body_length` names `warning` and `error`. The child
states each of the four.

Measured against a project configuration that states `disabled_rules:
[function_body_length, closure_body_length]` and `function_body_length:
warning: 500`, over one file holding one function of 302 body lines: the run
reports 1 finding. The same run with both option blocks removed from the script
reports 0.

## Generated code, which the project's own `excluded:` list carves out

`function-length` carves out generated code. swiftlint holds no generated-code
check of its own: it reads no header line and no file name. A project names the
directory its generator writes into in the `excluded:` list of its own
`.swiftlint.yml`, and that list is the whole carve-out.

Measured over two files that each hold one function of 302 body lines, one under
`Generated/` and one under `Sources/`, beside a `.swiftlint.yml` that states
`excluded: [Generated]`:

| the run | findings |
|---|---|
| the shipped script | 1 |
| the same script with `--force-exclude` removed | 2 |
| the same script that never names the project configuration | 2 |

An author cannot answer this carve-out with the directive below. The generator
writes the file again, and the directive goes away each time. That is why the
run makes the test and the author does not.

## A run whose every file the project excludes

`--force-exclude` can leave swiftlint no file to read. swiftlint then exits 1
and writes `Error: No lintable files found at paths: ...` to stderr, which reads
as a broken tool. A change that touched generated code alone would then answer
with a tool error rather than with a clean list.

The script reports nothing and exits 0 for that message. Measured over one file
under `Generated/` beside `excluded: [Generated]`: the run reports no finding,
exits 0, and writes swiftlint's own message to stderr.

A run that reports nothing and exits 0 over a file swiftlint never read is the
clean answer of a run that read every file. So the script states each path of
the run under the `sah-diagnostic:` marker before it exits, and the marked
line reads `function-length-swift judged no file at <path>, so its
bodies are unread`. Measured over the same file: no finding, ONE marked
line that names the path, exit 0.

A sound run says nothing on stderr, which is what lets the whole channel carry
the statement. Measured over the same file with no project configuration:
1 entry on stdout and 0 bytes on stderr.

The loop states a path that IS there, because the `[ ! -e "$file" ]` test above
already states a path that holds no file. Measured over `Sources/Absent.swift`
beside the excluded file: 2 marked lines, one for each path, and neither path
stated twice.

The acceptance test
`the_shipped_swift_function_length_tool_rule_declines_a_run_the_project_excludes_whole`
holds the marked line.

The message names the path and it does not name the cause, so more than one
shape reaches it. The section "A path the run cannot judge" below states each
shape, and states what the script says for a path that holds no file.

## Each stderr reading takes swiftlint's own message, and not a file name

The script reads stderr three times: a test for a project configuration
swiftlint cannot read, a substitution that states each file swiftlint could not
decode, and a test for a run that found no file to lint. swiftlint writes the
PATH of a file into stderr as well, so a reading that takes ALL of stderr
answers the file NAME.

swiftlint writes each message of its own at the START of a line, and it writes
the path echo after `Error: `. Each of the three readings is therefore anchored
on the start of a line, and the decode substitution carries the backtick pair
swiftlint writes around the path as well:

| what the script reads | the line swiftlint writes |
|---|---|
| `^Could not read configuration:` | `Could not read configuration: file Configuration.swift, line 278` |
| ``^Could not read contents of `<path>`$`` | ``Could not read contents of `<path>` `` |
| `^Error: No lintable files found at paths:` | `Error: No lintable files found at paths: '<path>'` |

Measured with swiftlint 0.65.0, over one file that holds one function of 302
body lines under `Generated/`, beside a project `.swiftlint.yml` that states
`excluded: [Generated]`. The file NAME is the one difference between the rows.
The loose script is the earlier shape of this run with each of its three tests
written as a plain `grep -qF` that carries no anchor and no closing punctuation:

| the file name | the loose script | the shipped script |
|---|---|---|
| `Staged.swift` | 0 findings, exit 0 | 0 findings, exit 0, 1 diagnostic |
| `Could not read contents of.swift` | 0 findings, exit 1, the rule's tool-error line | 0 findings, exit 0, 1 diagnostic |
| `Could not read configuration.swift` | 1 finding on a file the project excludes | 0 findings, exit 0, 1 diagnostic |
| `No lintable files found.swift` | 0 findings, exit 0 | 0 findings, exit 0, 1 diagnostic |

Row 2 broke a run that measured correctly. Row 3 made a WRONG FINDING: the
script dropped the project configuration, ran swiftlint a second time without
it, and reported a file the project excludes. Row 4 moved nothing, because the
decode reading stands above that test, and each other broken shape this rule
measures — a version mismatch and a configuration abort — writes no path into
stderr. The anchor holds each reading on swiftlint's own message as well.

## A project configuration swiftlint cannot read beside this rule

swiftlint reads the two `--config` paths as one hierarchy, and two shapes of the
project file stop it. Measured over one file that holds one function of 302 body
lines:

| the project `.swiftlint.yml` | what swiftlint does |
|---|---|
| `child_config: other.yml` | aborts, exit 134, `There's an ambiguity in the child / parent configuration tree` |
| bytes that are not YAML | aborts, exit 134, `Cannot parse YAML file` |

Each abort writes `Could not read configuration: file Configuration.swift, line
278` to stderr, and leaves stdout empty. A script that read that as a broken
tool exited 1. Both shapes are configurations swiftlint reads on its own, so a
project switched the gate off without meaning to.

The script tests stderr for `Could not read configuration:` at the start of a
line, and it then runs a second time with its own configuration alone. The
script writes one line to stderr that names what it dropped, under the
`sah-diagnostic:` marker. The run then measured with settings the project did
not ask for, which is one item it could not judge as the project asked, and
`builtin/validators/README.md` states that channel. The project's
`excluded:` list is not read for that second run. Measured over one file under
`Generated/` that holds the same function, beside a project file that states
`child_config: other.yml` and `excluded: [Generated]`: the run reports 1
finding, and swiftlint exits 2.

`parent_config:` in the project file is not one of the two shapes. Measured with
`parent_config: other.yml` beside the same file: swiftlint reads both
configurations and reports 1 finding.

The acceptance test
`the_shipped_swift_function_length_tool_rule_declines_a_project_configuration_it_cannot_read`
holds the marked line.

## A run cannot answer zero for a broken tool

swiftlint exits 1 for a file that is not there, and it writes nothing to stdout.
A shell pipeline takes the exit status of its LAST command, and that command was
`jq`, so an earlier pipe exited 0 and reported nothing. That reads exactly like
a clean file.

The script writes swiftlint's report to a file rather than into a pipe, so the
status of swiftlint reaches the gate above. A path the run cannot judge is
another shape, and it is not a broken tool: the section "A path the run cannot
judge" below states each shape, and states the marked line the script writes for
it at exit 0.

## A path the run cannot judge

A `files`-scope run is handed paths, and the engine reads the work-list rather
than the disk, so a path can reach the run and refuse it. Each way it refuses is
ONE item of a run that judged the other files, and
`builtin/validators/README.md` states the channel: a line opening
`sah-diagnostic:` on stderr, at exit 0.

Measured with swiftlint 0.65.0 against the child configuration this script
writes, over `Sources/Judged.swift`, which holds one function of 300 body lines,
beside each refusing path:

| the refusing path | status | stdout | stderr |
|---|---|---|---|
| a path that holds no file | 2 | 1 entry | 0 bytes |
| a file whose bytes are not UTF-8 | 2 | 1 entry | ``Could not read contents of `<path>` `` |
| a file with no read permission | 2 | 1 entry | the same decode line |

Each row carries the status and the report of a healthy run, and swiftlint
judged `Judged.swift` in every one of them. The child states `error: 250` beside
`warning: 250`, so a finding of this rule reaches error severity and swiftlint
exits 2 for it, which the section "What the script accepts at status 2" above
states. That is the one place this rule differs from `missing-docs-swift` and
`magic-numbers-swift`, whose child states no `error:` list and whose same three
rows therefore stand at exit 0. So neither the status nor the report states a
refusing path, and the run holds 1 finding that a nonzero exit would throw away.

swiftlint names the file for rows 2 and 3, and it says NOTHING for row 1.
Measured over the same run with `--quiet` taken off: swiftlint writes
`Linting Swift files at paths Sources/Judged.swift, Sources/Unreadable.swift`,
then `Linting 'Judged.swift' (1/1)`, and no word of the path it dropped. Row 1
therefore takes a test of the path, and rows 2 and 3 take a reading of
swiftlint's own message:

- `[ ! -e "$file" ]` runs before swiftlint, and it writes
  `sah-diagnostic: function-length-swift found no file at <path>, so its bodies
  are unread`.
- The `sed` substitution takes swiftlint's own decode line and writes
  `sah-diagnostic: swiftlint could not read the contents of <path>, so its
  bodies are unread`. swiftlint writes the ABSOLUTE path into that line, so the
  marked line carries one.

Measured with the shipped script, each refusing path beside `Judged.swift`: 1
finding on stdout, ONE marked line on stderr, exit 0. Measured over each
refusing path alone: no finding, the same one marked line, exit 0. Measured over
`Judged.swift` beside all three paths: 1 finding, 3 marked lines, exit 0.

`exit 1` is the answer this rule used to give, and it was wrong twice over.
Measured over `Judged.swift` beside each path:

| the refusing path | the earlier shape | the shipped script |
|---|---|---|
| a path that holds no file | 0 findings, exit 1, `function-length-swift cannot read Sources/Unreadable.swift` | 1 finding, exit 0, 1 diagnostic |
| a file whose bytes are not UTF-8 | 0 findings, exit 1, the rule's tool-error line | 1 finding, exit 0, 1 diagnostic |
| a file with no read permission | 0 findings, exit 1, `function-length-swift cannot read Sources/Unreadable.swift` | 1 finding, exit 0, 1 diagnostic |

Every row of the earlier shape threw the 1 finding away, which is what
`builtin/validators/README.md` refuses: a nonzero exit fails the WHOLE run, so
one unjudged path throws away every finding the run did make.

`[ ! -r "$file" ]` was the earlier test, and it cannot answer all three rows.
Measured against the three staged paths: the test is true for the path that
holds no file and for the file with no read permission, and FALSE for the file
whose bytes are not UTF-8 — the mode lets a reader open that one. So a run gated
on that test alone reads the third file as clean.

swiftlint reads a source file as UTF-8 and as nothing else, which is what makes
rows 2 and 3 one message. A Swift file a person saved in Latin-1, a binary file
under a `.swift` name, and a file whose mode refuses a read each make swiftlint
write ``Could not read contents of `<path>` ``, and swiftlint then lints no line
of that file. Measured over each of those two files ALONE, with no healthy file
beside it: swiftlint writes an empty array of 5 bytes to stdout, writes the same
decode line, and exits 0 — the status and the report of a clean file. So the
reading of stderr is the whole answer for those two rows.

A path that holds no file, alone, is the one refusing shape swiftlint answers
with a status of its own: it writes 0 bytes to stdout, writes
`Error: No lintable files found at paths: '<path>'` to stderr, and exits 1. The
script reports no finding and exits 0 for that message, which the section "A run
whose every file the project excludes" above states, and the marked line the
`[ ! -e "$file" ]` test wrote stands beside it.

Three acceptance tests hold the three rows, one for each —
`the_shipped_swift_function_length_tool_rule_declines_a_path_that_holds_no_file`,
`..._declines_a_file_it_cannot_decode` and `..._declines_a_file_it_may_not_read`.
Each stages `Sources/Judged.swift` beside the path, and holds the run to
reporting its 1 finding AND to stating one diagnostic that names the path. A run
that lost either half fails them.

The decode substitution is anchored on the start of a line, because swiftlint
writes a path into stderr as well. The section "Each stderr reading takes
swiftlint's own message, and not a file name" above states each row of that
measurement.

## A file that does not parse

swiftlint states no parse failure. It parses with recovery, and it lints the
declarations it recovered. So the script has no signal to read for this shape,
and the rule states the gap rather than a test it cannot make.

Measured with swiftlint 0.65.0, over one file that holds one function of 302
body lines under a broken head line: the run reports the finding for `@@@`, for
`(((`, for `]]]`, for `((( ]]]`, for `class {`, for `#if` and for
`this is not swift`, and it reports nothing for `@@@ this is not swift ((( ]]]`.
The last one is the shape the parser recovers nothing from. swiftlint writes an
empty array to stdout, writes 0 bytes to stderr, and exits 0. That is the answer
of a clean file, and no swiftlint flag states the difference. `swiftc -parse`
does state it — measured, it exits 1 for that file and 0 for the plain one — and
this rule does not run it, for two measured reasons. `swiftc -parse` also exits
1 for a file whose body never closes, which swiftlint measured correctly, so a
`swiftc` gate would trade a true finding for the shape. And `swiftc` is a Swift
toolchain, which `doctor.check_command` does not name and which a Homebrew
swiftlint does not bring.

## A run answers for the files it is given, and for no other

`swiftlint lint` with no path argument walks the whole tree under the working
directory. A `files`-scope script that hands `"$@"` straight to swiftlint
therefore answers for every Swift file under the repository root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of two files that each hold one function of 302 body
lines, with no argument: without the guard the script reported 2 findings and
exited 0; with the guard it reports none and exits 0. The same script over the
two files reports 2.

## The rule declares no install commands

Homebrew is the supported way to install swiftlint, and it installs the current
version only, so a Homebrew command cannot pin one. Mint can pin one —
`mint install realm/SwiftLint@0.65.0` — but it builds swiftlint from source and
links the result into `~/.mint/bin`, which is not on the path, so the command
cannot make `check_command` pass. The `doctor.fix_hint` states
`brew install swiftlint` instead. `sah doctor` shows that hint as the fix; the
install lifecycle never runs it.

## The directive an author writes

Selection in the filter is attribution, not exemption. To exempt one
declaration, write `// swiftlint:disable:next <rule>` on the line DIRECTLY above
it, and name the rule the finding names:

    // swiftlint:disable:next function_body_length  one line for each field
    init() {

Measured with swiftlint 0.65.0, over one function of 302 body lines against the
gate of 250. Each of these spellings gives no finding:

- `// swiftlint:disable:next function_body_length` on the line above the `func`
  line.
- `//swiftlint:disable:next function_body_length` with no space after the `//`.
- `// swiftlint:disable:next function_body_length - a flat field list` and
  `// swiftlint:disable:next function_body_length one line for each field`, each
  with a reason after the rule name.
- `// swiftlint:disable:next function_body_length closure_body_length`, which
  names the two rules of this gate.
- `// swiftlint:disable:next all`.
- `// swiftlint:disable function_body_length`, which runs to the end of the file
  or to the matching `// swiftlint:enable`.
- `// swiftlint:disable:this function_body_length` on the `func` line itself.
- `// swiftlint:disable:previous function_body_length` on the line UNDER the
  `func` line.
- a doc line above the directive.

Each of these spellings gives one finding:

- the directive with a blank line between it and the `func` line.
- the directive with a doc line between it and the `func` line.
- `// SwiftLint:disable:next function_body_length` with capital letters.
- `/* swiftlint:disable:next function_body_length */` as a block comment.
- `// swiftlint:disable:previous function_body_length` on the line ABOVE the
  `func` line, which names the line above the comment.
- `// swiftlint:disable:next closure_body_length` above a declaration the
  DECLARATION gate reports. A rule name the directive does not hold is not
  silenced.
- `// swiftlint:disable:next not_a_rule`, and the directive with no rule name at
  all.
- `// noqa: function_body_length`.

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
because that directive names another rule. Measured over a SwiftUI `body` of 300
`Text` rows in a `VStack`: the directive above the `VStack {` line gives no
finding, and the same directive above the `var body` line gives one.

The first fix a finding asks for is still to split the declaration. The
directive is the second fix, and the text beside it states why.

To exempt a whole directory, add it to the `excluded:` list of the project's own
`.swiftlint.yml`. The section "Generated code" above states that list.

## The carve-outs the prompt rule states

`function-length` exempts four shapes: a test, generated code, a function that
is mostly configuration or data, and an initialization function that sets many
fields. The run reproduces generated code, through the project's own `excluded:`
list. The author answers every other one with the directive above.

No swiftlint rule of this gate holds an option for any of them.
`swiftlint rules function_body_length` and `swiftlint rules closure_body_length`
each name `warning` and `error`. No option of the two reads a declaration name,
a superclass, a file header or a data line.

### A test, which the run does not drop, and why the definition cannot be read

`function-length` exempts "Functions explicitly marked as tests", and this set
names the DEFINITION as the mark: identify a test from its attribute or
framework naming convention at the definition, never from the file name. A
complex helper named `build_request` in a file called `foo_test.rs` is still a
long function and is still listed.

Swift states that convention in XCTest: a test is a method whose name starts
with `test`, in a class that subclasses `XCTestCase`. BOTH halves are the
definition, the same way `go test` needs both halves and
`function-length-go` reads both.

swiftlint offers no option that reads either half, and it writes no declaration
NAME into its message either — `Function body should span 250 lines or less
excluding comments and whitespace: currently spans 302 lines` carries the KIND
of the declaration and nothing more. That is where this rule differs from
`function-length-go`, which reads the name funlen writes into its message, and
from `function-length-python`, which reads the name ruff anchors its diagnostic
on.

The finding does anchor on the declaration line, so a filter could read
`func test…` out of the source. That read states one half of the definition and
not the other, and the missing half is the one that matters: `testConnection()`
on a database client is an ordinary method, and a name-only filter would silence
it. The Python rule reads a name alone because Python's own convention IS a name
alone; Swift's is not.

The corpus states what the gap costs, and it is nothing. 3591 of the 9807
declarations measured are named `func test…`, and the longest of them runs 239
lines — under the gate of 250. So no test of the corpus reports at all:

| `warning` and `error` | findings | named `func test…` |
|---|---|---|
| 100 | 39 | 16 |
| 150 | 8 | 2 |
| 200 | 4 | 2 |
| 250 | 1 | 0 |

Measured with swiftlint 0.65.0 over one file that holds a `func testEndToEnd()`
of 302 body lines inside an `XCTestCase` subclass, beside a `func
buildRequest()` of 302 body lines: the run reports both.

So a long test method REPORTS, and the author answers it. The first answer is to
move the table walk out of the test. The second answer is
`// swiftlint:disable:next function_body_length` above the method: measured, the
same method then reported nothing.

### Configuration, data and an initializer, which the run does not drop

`function-length` exempts "Functions that are mostly configuration/data (e.g.,
builder patterns with many options)" and "Initialization functions that set many
fields". `function_body_length` counts a data line like a code line.

Measured with swiftlint 0.65.0, at the gate of 250:

| the shape | the message |
|---|---|
| an `init` that sets 260 fields | `Initializer body should span 250 lines or less ... currently spans 260 lines` |
| a builder of 300 `.opt(n)` lines | `Function body ... currently spans 301 lines` |
| a dictionary of 300 entries in a `func` | `Function body ... currently spans 302 lines` |

So a data function and a long initializer REPORT, and the author answers them.
The first answer is to move the data out of the declaration — a `let` table, a
default value for each field, a smaller builder. The second answer is
`// swiftlint:disable:next function_body_length` above the declaration, with the
reason beside it. `function-length-rust` and `function-length-python` each
record the same verdict for the same carve-out.
