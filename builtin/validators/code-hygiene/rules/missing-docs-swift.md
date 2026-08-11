---
name: missing-docs-swift
description: Public Swift declarations need docs — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: missing-docs
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    for file in "$@"; do
      if [ ! -r "$file" ]; then
        printf 'missing-docs-swift cannot read %s\n' "$file" >&2
        exit 1
      fi
    done
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' 'only_rules:' '  - missing_docs' \
      'missing_docs:' '  warning: [open, public]' \
      '  excludes_extensions: true' '  excludes_inherited_types: true' \
      '  excludes_trivial_init: false' \
      '  evaluate_effective_access_control_level: false' > "$work/swiftlint.yml"
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
      printf '%s\n' 'missing-docs-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.' >&2
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
    jq -c '.[] | select(.rule_id == "missing_docs")
           | {file: .file, line: .line, message: .reason}' "$work/report.json"
  doctor:
    check_command: "which swiftlint jq grep mktemp"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Missing Documentation — Swift

`swiftlint` reports every `open` and `public` declaration without a doc
comment. The `missing_docs` rule names that check. It is opt-in, so it never
runs unless a configuration turns it on.

The fail fixture holds one undocumented public structure, one undocumented
public method and one undocumented public function. The acceptance test
`the_shipped_swift_missing_docs_tool_rule_reports_every_fail_fixture_line`
holds swiftlint to reporting those three lines and no other.

Every measurement below was made with swiftlint 0.65.0.

## How the run is shaped

The script names TWO configuration files. swiftlint reads a list of `--config`
paths as a parent-child hierarchy.

- The PARENT is the project's own `.swiftlint.yml` at the repository root. The
  script names it only when the file is there, because a `--config` path that
  holds no file aborts swiftlint: measured, exit 134 with
  `Could not read configuration`. The parent gives the run the project's
  `excluded:` list.
- The CHILD is the file the script writes into a temporary directory. It
  states `only_rules: [missing_docs]` and every option of that rule, so the
  rule owns what it measures.

`--force-exclude` makes swiftlint apply the `excluded:` list to a file named as
a command-line argument. `--no-cache` keeps swiftlint from writing a cache
directory into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

## Generated code, which the project's own `excluded:` list carves out

The `missing-docs` prompt rule carves out generated code. swiftlint holds no
generated-code check of its own: it reads no header line and no file name. A
project names the directory its generator writes into in the `excluded:` list
of its own `.swiftlint.yml`, and that list is the whole carve-out.

Measured over two files that hold the same undocumented `public struct` and the
same undocumented stored property, one under `Generated/` and one under
`Sources/`, beside a `.swiftlint.yml` that states `excluded: [Generated]`:

| the run | findings |
|---|---|
| the shipped script | 2 |
| the same script with `--force-exclude` removed | 4 |
| the same script that never names the project configuration | 4 |

An `excluded:` entry resolves against the directory of the configuration file
that states it, so the entry has to reach the project tree from a file under
`/var/folders`. Measured over the same two files, with `excluded:` written into
the temporary configuration beside the shipped rule set:

| the entry the temporary configuration states | findings |
|---|---|
| `Generated`, with `--force-exclude` | 4 |
| the absolute path of `Generated`, with `--force-exclude` | 2 |
| the absolute path of `Generated`, with `--force-exclude` removed | 4 |

So the script could write the exclude list itself, in absolute form. It would
then have to read the project's YAML with a tool of its own, and it would read
the root file alone. swiftlint reads that YAML already, so the script names the
project's file and lets swiftlint do it.

The acceptance test
`the_shipped_swift_missing_docs_tool_rule_reads_the_project_exclude_list`
holds the run to the 2 findings of the first row.

The script reads the `.swiftlint.yml` at the repository root and no other file.
Measured with a `Nested/.swiftlint.yml` that states `excluded: ['.']` and one
file under `Nested/`: the run reports that file. `--config` turns swiftlint's
own directory-by-directory lookup off.

An `included:` list changes nothing. Measured with `included: [Sources]` in the
project configuration and one file under `Tests/`: the run reports that file.

## A run whose every file the project excludes

`--force-exclude` can leave swiftlint no file to read. swiftlint then exits 1
and writes `Error: No lintable files found at paths: ...` to stderr, which
reads as a broken tool. A change that touched generated code alone would then
answer with a tool error rather than with a clean list.

The script tests each file it is given for readability before it starts, so
that message can carry one cause only: the exclude list took every file. The
script reports nothing and exits 0. Measured over one file under `Generated/`
beside `excluded: [Generated]`: the run reports no finding, exits 0, and
writes swiftlint's own message to stderr. The acceptance test
`the_shipped_swift_missing_docs_tool_rule_answers_zero_when_the_project_excludes_every_file`
holds that behaviour.

## A project configuration swiftlint cannot read beside this rule

swiftlint reads the two `--config` paths as one hierarchy, and two shapes of
the project file stop it. Measured over one file that holds an undocumented
`public struct` and one undocumented stored property:

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
same declarations, beside a project file that states `child_config: other.yml`
and `excluded: [Generated]`: the run reports 2 findings and exits 0.

`parent_config:` in the project file is not one of the two shapes. Measured
with `parent_config: other.yml` beside the same file: swiftlint reads both
configurations and exits 0.

The acceptance test
`the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_child_config`
holds that behaviour.

## A project warning threshold, and what the script accepts at status 2

swiftlint counts the warnings of a run against the `warning_threshold:` key of
the project configuration. At that number, and over it, swiftlint adds one
entry of `rule_id: warning_threshold` and error severity to the report, and it
exits 2. Every finding of the run stands on stdout beside that entry.

Measured over one file that holds an undocumented `public struct` and one
undocumented stored property:

| the project `.swiftlint.yml` | swiftlint | the script |
|---|---|---|
| no file | exit 0, 2 entries | 2 findings, exit 0 |
| `warning_threshold: 5` | exit 0, 2 entries | 2 findings, exit 0 |
| `warning_threshold: 1` | exit 2, 3 entries | 2 findings, exit 0 |

The script tested `[ "$status" -ne 0 ]`, so it read status 2 as a broken tool.
It then reported 0 findings and exited 1 for the third row, and the engine read
that exit as a broken tool. One line in the project file switched the gate off.
The script now accepts status 2, and the third row keeps the 2 findings of the
first row.

The `jq` filter selects `rule_id == "missing_docs"`, so the `warning_threshold`
entry never becomes a finding.

The status alone does not tell a measured run from a broken run. At status 2
the REPORT tells the two apart: the threshold run writes a JSON array, and the
version-mismatch run writes 0 bytes. The report makes that one distinction. At
status 1 the report is 0 bytes for the clean run beside a project `excluded:`
list, and 0 bytes for the broken run over a path that holds no file. Each
status swiftlint 0.65.0 answers with was measured against the child
configuration this script writes:

| what the run is | status | stdout |
|---|---|---|
| a file whose every public item carries a doc comment | 0 | an empty array, 5 bytes |
| one file that holds 2 undocumented public items | 0 | 2 entries, 726 bytes |
| the same file beside `warning_threshold: 1` | 2 | 3 entries, 949 bytes |
| the same file beside `swiftlint_version: 99.0.0` | 2 | 0 bytes |
| the same file beside a project `excluded:` that covers it | 1 | 0 bytes |
| one file whose only line is `public func oops( {` | 0 | 1 entry, 364 bytes |
| a path that holds no file | 1 | 0 bytes |
| a `--config` path that holds no file | 134 | 0 bytes |
| a project configuration that holds `child_config:` | 134 | 0 bytes |
| a command-line option that does not exist | 64 | 0 bytes |

The two runs of status 2 differ in the report. The threshold run wrote 3
entries. The version run wrote 0 bytes and linted no file: swiftlint compares
`swiftlint_version:` with the version it is, and at a difference it writes
`warning: Currently running SwiftLint 0.65.0 but configuration specified
version 99.0.0.` to stderr and stops. Each run that measured wrote a JSON
array, at status 0 or 2. Each other run wrote 0 bytes, at status 1, 134, 64 or
2. A report of 0 bytes does not make a run broken. The run beside a project
`excluded:` that covers the file writes 0 bytes at status 1, and it gives a
clean answer. The guard on each file and the test on stderr separate that run
from a run that broke. The two paragraphs below state each test and its limit.

So the script accepts status 0, and it accepts status 2 only when the report
holds a JSON array of one entry or more. At each other status, and at status 2
with a report of 0 bytes, the script makes one more test, on stderr. Stderr
that holds `No lintable files found` exits 0 with no finding, and each other
shape exits 1. That branch is how the project's `excluded:` list reaches a
clean answer, and the section "A run whose every file the project excludes"
above states it. Measured with a project `.swiftlint.yml` that states
`excluded: [src]`, over one file under `src/` that holds an undocumented
`public struct` and one undocumented stored property: swiftlint writes
`Error: No lintable files found at paths: 'src/Docs.swift'` to stderr, writes 0
bytes to stdout, and exits 1; the script reports no finding and exits 0.

The stderr string names the path, and it does not name the reason. Measured
against the child configuration this script writes, 4 shapes each wrote 0
bytes at status 1 with the string `No lintable files found`: a project
`excluded: [src]` list over `src/Docs.swift`; the directory `hollow`, which
holds no Swift file; the path `src/Absent.swift`, which holds no file; the
file `src/Notes.txt`, whose name does not end in `.swift`. The script reports
0 findings and exits 0 for 3 of the 4 shapes. The `[ ! -r "$file" ]` guard
runs before swiftlint, and it reports 0 findings and exits 1 with
`missing-docs-swift cannot read src/Absent.swift` for the path that holds no
file. That guard makes that one distinction, and no test separates the other
3 shapes.

Measured over one file that holds an undocumented `public struct` and one
undocumented stored property, beside a project `.swiftlint.yml` that states
`swiftlint_version:`: at `0.65.0` the script reports 2 findings and exits 0; at
`0.64.0`, at `99.0.0` and at `0.1.0` the script reports 0 findings and exits 1,
which the engine reads as a broken tool. A script that accepted every status 2
reported 0 findings and exited 0 for each of those three values, and the engine
read a dirty file as clean.

`warning_threshold:` and a finding of error severity are the two shapes that
make swiftlint exit 2 with a report of one entry or more, and a project cannot
reach the second. The child states `warning: [open, public]` and no `error:`
list, and a child block replaces the parent block whole. Measured with a
project configuration that states `missing_docs:` with
`error: [open, public]`, over the same file: swiftlint exits 0 and writes 2
entries of warning severity. Measured with a child that states the same
`error:` list instead: swiftlint exits 2 and writes 2 entries of error
severity.

The acceptance test
`the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_warning_threshold`
holds the run to the 2 findings of the third row of the first table. The
acceptance test
`the_shipped_swift_missing_docs_tool_rule_breaks_beside_a_project_version_mismatch`
holds the run beside `swiftlint_version: 99.0.0` to no finding and one tool
error.

## The rule owns its own options

A project configuration can state options for `missing_docs`, and the parent's
block reaches the run when the child states none. Measured with a parent that
states `excludes_inherited_types: false` and a child that states no
`missing_docs:` block, over one undocumented `public struct Wide: Equatable`
holding one undocumented stored property: the run reports 2 findings.

The child's `missing_docs:` block replaces the parent's block whole, so the
script writes every option the rule has. Each value is swiftlint's own default,
written out so the project cannot change it:

| option | the value the script states |
|---|---|
| `warning` | `[open, public]` |
| `excludes_extensions` | `true` |
| `excludes_inherited_types` | `true` |
| `excludes_trivial_init` | `false` |
| `evaluate_effective_access_control_level` | `false` |

Measured against a project configuration that states `disabled_rules:
[missing_docs]`, `warning: [open]` and `excludes_inherited_types: false`, over
one file that holds an undocumented `public struct Wide: Equatable` with one
stored property and an undocumented `public struct Plain` with one stored
property: the run reports the 2 rows of `Plain` and no other. The same run with
the option block removed from the script reports 0. The acceptance test
`the_shipped_swift_missing_docs_tool_rule_keeps_its_own_rule_options` holds
that pair of counts.

A project cannot switch the rule off. `only_rules` in the child beats
`disabled_rules` in the parent. Measured with a parent that states
`disabled_rules: [missing_docs]` and no option block, over one file that holds
an undocumented `public struct` and one undocumented stored property: the run
reports 2 findings.

## What swiftlint carves out for itself

Measured on one probe file that holds each shape below and each shape of the
section under this one. The run reports 7 findings, at rows 22, 25, 26, 27, 28,
35 and 36. Each shape here reports nothing.

- **A private or internal declaration.** `struct InternalStructure` with an
  internal stored property, `private struct PrivateStructure` with a private
  stored property, and `func internalFunction()` each report nothing.
  `warning: [open, public]` is the cause. This is the prompt rule's private
  carve-out, reproduced by the option.
- **A protocol conformance written in an extension.** `extension Documented:
  CustomStringConvertible` and the `public var description` inside it report
  nothing. The extension declares an inherited type, so
  `excludes_inherited_types: true` passes over it. Measured: the same run with
  that option set to `false` reports row 18, the `public var description`. This
  is the prompt rule's "Obvious implementations (Display, Debug, ToString,
  etc.)" carve-out, reproduced by the option.
- **An extension declaration.** `public extension Documented` at row 21 reports
  nothing. Measured: the same run with `excludes_extensions: false` reports row
  21. The MEMBER inside that extension still reports, and it is row 22 of the
  seven.
- **An XCTest class and its test method.** `final class ThingTests:
  XCTestCase` and the `func testThing()` inside it report nothing. Both are
  internal, so `warning: [open, public]` is the whole cause. swiftlint gives
  `missing_docs` no test option at all.

## What swiftlint does not carve out

Each shape here is one of the 7 findings above.

- **A getter and a setter, inside a type that declares no inherited type.** The
  `missing-docs` prompt rule carves out "Simple getters/setters with
  self-explanatory names". Measured: the undocumented
  `public var value: Int { 1 }` at row 27 and the undocumented `public func
  setValue(_ next: Int)` at row 28 each report. Both stand inside a type that
  declares no inherited type, and the inherited type decides. Measured on a
  second probe of two files: one holds `public struct Plain` with the same two
  undocumented items, and the run reports 3 findings, at rows 1, 2 and 3; the
  other holds `public struct Wide: Equatable` with the same two undocumented
  items, and the run reports 0 findings. `excludes_inherited_types: true`
  passes over the whole of `Wide`. So a public getter and a public setter each
  need a doc comment inside a type that declares no inherited type.
  `missing-docs.md` states the same condition.
- **A trivial initializer.** The undocumented `public init() {}` at row 26
  reports. `excludes_trivial_init: false` is the cause. Measured: the same run
  with that option set to `true` reports 6 findings and drops row 26.
- **A public helper inside a test file.** `public final class TestSupport` at
  row 35 and its `public func makeThing()` at row 36 each report. The
  `missing-docs` prompt rule asks for exactly this: "Identify test items from
  the structural marker on the item itself ... not from the file name or path."
  swiftlint reads the access level and never the path, so the test carve-out
  holds for an internal test and holds for nothing else.

`evaluate_effective_access_control_level` moved no row of this probe. Measured:
the same run with that option set to `true` reports the same 7 rows.

The recourse for any of these is the inline suppression at the end of this
file.

## A run cannot answer zero for a broken tool

swiftlint exits 1 for a file that is not there, and it writes nothing to
stdout. A shell pipeline takes the exit status of its LAST command, and that
command was `jq`, so the earlier pipe exited 0 and reported nothing. That reads
exactly like a clean file.

The script tests each file it is given before it starts, and it writes
swiftlint's report to a file rather than into a pipe. Measured over one path
that holds no file: the earlier pipe reported no finding and exited 0; the
script reports no finding and exits 1, with `missing-docs-swift cannot read
Sources/Absent.swift` on stderr. The acceptance test
`the_shipped_swift_missing_docs_tool_rule_breaks_on_a_file_it_cannot_read`
holds that behaviour.

A Swift file that does not parse is not one of these. Measured over one file
that holds `public func oops( {`: swiftlint recovers from the parse error,
reports 1 `missing_docs` finding and exits 0. That count agrees with the row
of the status table above. swiftlint states no parse failure of its own, so
no filter can drop one.

`mktemp -d` makes the working directory the script writes the configuration and
the report into, and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers
every way the script leaves: a clean run, a finding, and a failure.

## A run answers for the files it is given, and for no other

`swiftlint lint` with no path argument walks the whole tree under the working
directory. A `files`-scope script that hands `"$@"` straight to swiftlint
therefore answers for every Swift file under the repository root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of `Top.swift` and `deep/nested/Other.swift`, with no
argument: without the guard the script reported 4 findings over those two files
and exited 0; with the guard it reports none and exits 0. The acceptance test
`the_shipped_swift_missing_docs_tool_rule_reads_only_the_files_it_is_given`
holds that behaviour.

## The rule declares no install commands

Homebrew is the supported way to install swiftlint, and it installs the current
version only, so a Homebrew command cannot pin one. Mint can pin one —
`mint install realm/SwiftLint@0.65.0` — but it builds swiftlint from source and
links the result into `~/.mint/bin`, which is not on the path, so the command
cannot make `check_command` pass. The `doctor.fix_hint` states
`brew install swiftlint` instead. `sah doctor` shows that hint as the fix; the
install lifecycle never runs it.

## How to exempt one declaration

Selection in the filter is attribution, not exemption: to exempt one
declaration, write `// swiftlint:disable:next missing_docs` above it in the
code. To exempt a whole directory, add it to the `excluded:` list of the
project's own `.swiftlint.yml`.
