---
name: missing-docs-go
description: Exported Go items need doc comments — checked by revive, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: missing-docs
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    pending="$#"
    while [ "$pending" -gt 0 ]; do
      file="$1"
      shift
      pending=$((pending - 1))
      if [ -e "$file" ] && [ ! -r "$file" ]; then
        printf 'sah-diagnostic: missing-docs-go cannot read %s, so its exported items are unread\n' "$file" >&2
        continue
      fi
      set -- "$@" "$file"
    done
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' '[rule.exported]' '  arguments = ["disableStutteringCheck"]' > "$work/revive.toml"
    revive -config "$work/revive.toml" -formatter json "$@" > "$work/revive.json"
    jq -r '(. // [])[] | select(.RuleName == "")
           | "sah-diagnostic: revive declined an item and said: \(.Failure)"' "$work/revive.json" >&2
    jq -c '(. // [])[] | select(.RuleName == "exported")
           | {file: .Position.Start.Filename, line: .Position.Start.Line, message: .Failure}' "$work/revive.json"
  doctor:
    check_command: "which revive jq mktemp"
    check_version_command: "revive -version"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/mgechev/revive@v1.15.0'
---

# Missing Documentation — Go

`revive` reports every exported type, method, function, constant and variable
without a doc comment. The `exported` rule names that check. Each of the five
kinds is measured: the fail fixture holds one of each, and the acceptance test
`the_shipped_go_missing_docs_tool_rule_reports_every_fail_fixture_item` holds
revive to reporting all five.

`exported` is stricter than the prompt rule it replaces, and the prompt rule
sanctions that: "These exemptions yield to stricter language-specific
documentation rules." Two shapes carry a comment and are still reported.

The script writes its own `revive.toml` to a temporary path and passes it with
`-config`. The config names one rule, which both turns that rule on and turns
every other revive rule off, so the rule owns its whole invocation and never
reads the project's own revive configuration.

The scope is `files` because revive reads the files it is given. It needs no
`go.mod` to lint a loose file.

Every measurement below was made on revive 1.15.0.

## The `exported` rule also reports a NAME, so the config turns that off

`exported` holds a second check. It reports an exported name that repeats its
own package name, because a caller then writes the word two times:
`staged.StagedType`. revive gives that finding the rule name `exported`, the
same rule name a documentation finding carries.

The two findings differ in the CATEGORY revive writes beside the rule name. A
documentation finding carries `comments`. A stuttering name carries `naming`.
Measured over one file that holds a documented stuttering type, an undocumented
plain type, an undocumented stuttering type and an undocumented stuttering
constant: the config without the setting reports 5 findings — three `comments`
and two `naming` — and the config with the setting reports the three `comments`
findings and no other. A documented stuttering type still reports its name. A
stuttering constant reports no name.

`disableStutteringCheck` is the revive setting that turns the name check off,
and the config states it. This rule supersedes the `missing-docs` prompt rule
and no other. `missing-docs` asks for a doc comment, and a stuttering name is
not a missing doc comment, so this rule does not own that defect. The config
asks revive for the one check the rule owns. A filter on the category would
guard a finding this config never makes, and no test could measure it.

**`stuttering-name-go` owns the stuttering Go name.** It runs the same revive
`exported` rule, states no argument at all, and selects the `naming` category
in its filter, so the two rules together are revive's whole `exported` output
with no finding owned two times and none dropped. Measured: 26 shipped rules
match a `.go` file, and `stuttering-name-go` is the one of them that reports a
NAME as the defect. The naming rules of the other languages —
`swift/naming-clarity`, `swift/doc-parameter-naming` and
`js-ts/naming-and-style` — read no `.go` file. The acceptance test
`the_shipped_rules_that_read_a_go_file_stay_the_stated_list` holds that list of
26, so a rule added later fails the test and the reader then decides, and
`the_shipped_go_rules_that_run_revives_exported_rule_split_its_findings` drives
both shipped scripts over one file and holds the split.

## A doc comment must open with the item's own name

`exported` holds a third check. A doc comment that does not open with the name
of the item it stands above reads as no documentation at all. That is the Go
convention, and the message states the form it wants.

Measured over one file, one item for each kind: `// does the thing` above
`func BadFunc()` reports `comment on exported function BadFunc should be of the
form "BadFunc ..."`. A comment in the wrong form above a type, a method, a
constant and a variable each report the same way. A type accepts a leading
article, so `// A GoodType is documented ...` stays silent.

The fail fixture holds one function of this shape, so the check stays measured.

## A `Deprecated:` note alone is not a doc comment

A comment that opens with `Deprecated:` is a deprecation note, and revive counts
it as no documentation. Measured: `// Deprecated: OnlyDeprecated is gone.` above
`func OnlyDeprecated()` reports `exported function OnlyDeprecated should have
comment or be unexported`. A doc comment with a `Deprecated:` paragraph under it
stays silent.

So a deprecated item keeps its doc comment and puts the note under it. The fail
fixture holds the reported shape, and the passing fixture holds the silent one.

## Generated code, which the default already carves out

The `missing-docs` prompt rule carves out generated code. revive carves out the
same code, and the SHIPPED config is what keeps the carve-out.

**The name of the revive option says the opposite of its effect.**
`ignoreGeneratedHeader = true` makes revive ignore the generated header, so
revive then reads a generated file as an ordinary file. The default is `false`,
which makes revive honour the header and read no generated file. The config
therefore leaves the option out.

Measured, one file for each row:

| file | the shipped config | `ignoreGeneratedHeader = true` |
|---|---|---|
| a `.pb.go` file with the header | 0 | 5 |
| a `.go` file with the header | 0 | 1 |
| a `.pb.go` file without the header | 2 | 2 |
| an ordinary `.go` file | 2 | 2 |

The carve-out reads the HEADER LINE, and it never reads the file name. The line
is `// Code generated ... DO NOT EDIT.`, the form the Go convention defines.
revive matches the whole line, so four properties hold, each one measured:

- The case must be exact. A lower case `// code generated ... do not edit.`
  reports.
- The line must end at `DO NOT EDIT.`. Text after it reports.
- The comment must be a line comment. A `/* ... */` block reports.
- The position of the line does not matter. The file stays silent with the line
  first, with the line under a licence comment, with no blank line under it, and
  with the line under the package clause.

Measured silent on the header of four generators: protoc-gen-go, MockGen, sqlc,
and `// Code generated by "stringer -type=Pill"; DO NOT EDIT.`

## Tests, which revive carves out by FILE NAME

`exported` reads no `_test.go` file. The skip is the file name, and it covers
the whole file.

Measured: a file named `a_test.go` that holds `type ExportedInTestFile`,
`func ExportedHelperInTestFile()` and `func TestSomething(t *testing.T)` reports
nothing. The same bytes under the name `a_testlike.go` report all three.

The `missing-docs` prompt rule asks for the opposite test: "Identify test items
from the structural marker on the item itself ... not from the file name or
path." revive gives no option that changes its own test, so the two disagree. A
helper in a `_test.go` file needs no doc comment under this rule, and this rule
supersedes the prompt rule for every `.go` file, so no rule asks for one. The
gap is stated here rather than left to be found.

## A command, which revive carves out by PACKAGE CLAUSE

`exported` reads no file of `package main`. A command exports nothing to a
caller outside itself, so the check has nothing to answer for.

Measured: one file that holds an undocumented exported type, method, function,
constant and variable reports 5 findings under `package library` and 0 findings
under `package main`. The package clause is the whole difference. A build tag
changes nothing: the same declarations under `//go:build ignore` still report
when the package clause is not `main`.

This is a second place where the prompt rule asks for more. The prompt rule
knows no package clause, and this rule supersedes it for every `.go` file, so an
exported item inside a command needs no doc comment.

## Obvious implementations, which revive carves out by NAME

The `missing-docs` prompt rule carves out "Obvious implementations (Display,
Debug, ToString, etc.)". revive carves out a fixed list of method names:
`Error`, `String`, `Read`, `Write`, `Unwrap` and `ServeHTTP`. The item must have
a receiver.

Measured on one type that holds 12 undocumented methods: revive reports 6 of
them — `Close`, `Len`, `MarshalJSON`, `GoString`, `Format` and an ordinary
method. The six names above report nothing. Two free functions named `Error` and
`String` both report, because a free function has no receiver.

The passing fixture holds one undocumented method for each of the six names, so
a revive release that drops a name from the list fails the fixture pair.

## What revive does not carve out

The `missing-docs` prompt rule carves out "Simple getters/setters with
self-explanatory names". revive has no setting for it. Measured: an undocumented
`func (a Accessors) Value() int` and the `func (a *Accessors) SetValue(next int)`
beside it each report.

`disableChecksOnMethods` is not the setting to reach for. It turns off EVERY
method check. Measured: the two findings above become none, and the undocumented
method of the fail fixture goes silent with them.

So a public getter and a public setter each need a doc comment. The fail fixture
carries one of each for that reason, and the acceptance test holds revive to
reporting them, so the gap stays measured. The recourse is the inline
suppression at the end of this file.

## Private items, which Go carves out by capitalization

`exported` reads an exported name and no other. Measured: an unexported type, an
unexported function and an unexported constant each report nothing. An exported
method on an UNEXPORTED receiver reports nothing as well, because
`checkPrivateReceivers` is off by default and the config leaves it off.

## A run cannot answer zero for an item it never measured

revive states a failure in two ways and stays silent for a third, and the
earlier pipe answered none of the three.

`builtin/validators/README.md` splits the answer in two. A script that judged
the code and could not judge ONE item writes a line opening `sah-diagnostic:`
on stderr and exits 0, and "a nonzero exit fails the WHOLE run, so one unjudged
path throws away every finding the run did make". So the measurement that
decides each shape is what revive reported for the OTHER files of the same run.

- **revive exits nonzero** for a path that holds no file, and for a config it
  cannot parse. A pipeline takes the exit status of its LAST command, and that
  command was `jq`, so the run exited 0 with no output. The script writes
  revive's report to a file rather than into a pipe, and `set -e` makes revive's
  own failure the exit status of the script.
- **revive exits 0 for a file it cannot read as Go source.** It writes the
  failure onto the SAME report as a finding, with an EMPTY `RuleName`. A filter
  that selects the `exported` findings drops that record, so the file read as
  clean.
- **revive exits 0 and stays SILENT for a path it can stat and cannot open.**
  It writes no record of that path on the report and 0 bytes on stderr, so
  neither channel names it. The subsection "A path revive drops before it
  lints" holds that shape.

Measured with revive 1.15.0, each shape staged BESIDE one file holding an
undocumented exported type:

| the run | what revive does with it | what the other file gets | what the script does |
|---|---|---|---|
| a Go file that does not parse | one unnamed record, exit 0 | its finding | states it, exit 0 |
| a file of 0 bytes | one unnamed record, exit 0 | its finding | states it, exit 0 |
| a file that is not Go source | one unnamed record, exit 0 | its finding | states it, exit 0 |
| a file whose bytes are not UTF-8 | one unnamed record, exit 0 | its finding | states it, exit 0 |
| a path that holds no file | 0 bytes on stdout, exit 1 | nothing | exits 1 |
| a config it cannot parse | 0 bytes on stdout, exit 1 | nothing | exits 1 |

The first four rows are one DECLINED ITEM of a run that stayed sound. revive
read every other file, and the finding of the file it read stands on the
report, so an `exit 1` would throw that finding away. The file it refused is
dropped from its package and every other file of that package is still linted,
so the two files need not stand apart: measured with both files in one package
and with each file in a package of its own, the other file reported its finding
both times.

Each of the four therefore writes one line that OPENS with `sah-diagnostic:`,
and the run exits 0. The marker opens the line because the engine reads a
marked line with `strip_prefix`.

    sah-diagnostic: revive declined an item and said: invalid file broken.go: broken.go:3:14: expected ')', found '{'
    sah-diagnostic: revive declined an item and said: invalid file empty.go: empty.go:1:1: expected 'package', found 'EOF'
    sah-diagnostic: revive declined an item and said: invalid file notgo.txt: notgo.txt:1:1: expected 'package', found plain
    sah-diagnostic: revive declined an item and said: invalid file nonutf8.go: nonutf8.go:3:9: illegal UTF-8 encoding (and 3 more errors)

The whole `Failure` is forwarded, and no head is read or stripped. Every
unnamed record this survey met carried the `validity` category and a sentence
opening `invalid file`, and the filter reads NEITHER of those: it selects the
record that belongs to no rule, so a record revive writes under another
category, or under another sentence, still reaches the marker.
`missing-docs-python` records the same lesson for ruff's own channel: a head
written into a rule answers for the one shape it was written for and stays
silent for every other.

A run that declines EVERY file it is given states each one and still exits 0.
Measured over two files that do not parse and no other: no finding, two marked
lines, exit 0. No item of that run reads as a clean pass, because the run
states each one.

The last two rows are a BROKEN run rather than a declined item. revive resolves
its paths and reads its config before it reads any file, so either shape costs
the whole run and leaves no finding to lose. Measured beside the same reporting
file: a path that holds no file writes 0 bytes to stdout, `cannot find package
"absent.go"` to stderr and exits 1, and the reporting file got no finding; a
config of `this is not toml [[[` writes 0 bytes to stdout, `cannot parse the
config file` to stderr and exits 1. `set -e` exits the script for both, so the
script states nothing of its own.

A sound run writes 0 bytes on stderr. Measured over the reporting file alone:
one finding on stdout, 0 bytes on stderr, exit 0.

### A path revive drops before it lints

Every row above reaches revive and comes back on its report. A path can refuse
the READER instead, and revive then answers for it in one of two ways. The line
between the two is the STAT: revive stats each path before it opens it.

Measured with revive 1.15.0, each shape staged beside the same reporting file:

| the path | `[ -e ]` | `[ -r ]` | revive | its stderr | the reporting file |
|---|---|---|---|---|---|
| a file at mode 000 | yes | no | no record of it, exit 0 | 0 bytes | its finding |
| a file at mode 200 | yes | no | no record of it, exit 0 | 0 bytes | its finding |
| a symlink to a file at mode 000 | yes | no | no record of it, exit 0 | 0 bytes | its finding |
| a directory nobody may read | yes | no | exit 1 | 106 bytes | nothing |
| a file under a directory nobody may read | no | no | exit 1 | 198 bytes | nothing |
| a dangling symlink | no | no | exit 1 | 190 bytes | nothing |
| a symlink loop | no | no | exit 1 | 182 bytes | nothing |
| a path that holds no file | no | no | exit 1 | 186 bytes | nothing |

The first three rows are the defect this subsection answers. revive drops the
path in SILENCE: no record of any category on the report, 0 bytes on stderr,
exit 0, and the file it read still reports its finding. The same path ALONE
writes `null` to stdout at exit 0, which is the report and the status of a
clean file. So neither the report nor the channel holds a thing to forward, and
the script tests each path ITSELF. That is the answer
`builtin/validators/README.md` names: "A tool can exit 0 for a file it could
not open, and print an empty report. Test each file the script is given before
the tool starts."

`[ -e "$file" ]` is the stat and `[ ! -r "$file" ]` is the open, so the two
tests together name the silent shape and no other. The script writes one marked
line for each such path, and it drops that path from the list it hands revive:

    sah-diagnostic: missing-docs-go cannot read forbidden.go, so its exported items are unread

Measured over the SHIPPED script, one file at mode 000 beside the file that
reports: the finding of that file on stdout, the one marked line above on
stderr, exit 0.

The five loud rows keep the answer the last two rows of the table above state.
Each one costs the whole run at exit 1, and `set -e` exits the script for it. A
directory nobody may read is a loud row that the two tests reach all the same,
because `[ -e ]` stats it and `[ -r ]` refuses it. The run then declines that
one path and judges every other, which is the better of the two answers.

A READABLE directory reaches revive as a package rather than as a file. revive
lints the Go files it holds, and an empty one holds nothing to judge, so it is
no unjudged item and `[ -r ]` admits it for that reason. Measured: an empty
directory beside the reporting file leaves that file's finding standing, makes
no record of the directory, and exits 0.

The count guard of the section below stands ABOVE this filter, so a run that
is given no file answers before the filter reads a path. The script counts its
arguments a SECOND time under the filter, because a run whose every path
refuses the reader has no file left to hand revive. Such a run writes one
marked line for each path and still exits 0 with no finding. Measured over the
shipped script with the file at mode 000 alone: no finding, one marked line,
exit 0.

The acceptance test
`the_shipped_go_missing_docs_tool_rule_declines_a_file_it_may_not_read` stages
the file that reports beside a file at mode 000, and holds the run to reporting
that finding AND to stating the one item it declined.

The acceptance test
`the_shipped_go_missing_docs_tool_rule_declines_a_file_it_cannot_parse` stages
the undocumented exported type beside the file that does not parse, and holds
the run to reporting that finding AND to stating the one item it declined. Both
halves are the test: a run that reported the finding and said nothing about the
file it refused reads that file as clean, and a run that stated the item and
lost the finding is the `exit 1` this section replaced.

`-formatter json` prints `null`, not an empty array, for a file with no
findings, so each `jq` filter starts with `(. // [])`.

## How to exempt one item

Selection in the filter is attribution, not exemption: to exempt one item, write
`//revive:disable-next-line:exported` above it in the code. Measured: the marker
on the line above silences the finding, and `//revive:disable:exported` inside a
file silences the whole file.

## The run answers for its own arguments

revive reads the package standing in the working directory when it takes
no path. A run with no file therefore reports an undocumented exported
item of the workspace root package, at exit 0, and it says nothing about a
package deeper in the tree. The script counts its arguments first, and a count
of zero exits 0 with no finding. That guard stands above every line that runs,
so it is the first answer the script gives. The script counts its arguments a
second time under the read-path filter above, because a run whose every path
refuses the reader has no file left to hand revive.

Measured over two Go files, each exporting one undocumented function, one
at the root and one three directories down, with no argument: 1 finding
before the guard, on the file at the root, and 0 after it. The same script
over the two files reports 2. The acceptance test
`the_shipped_go_missing_docs_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.

## The temporary directory the configuration stands in

`mktemp -d` makes the directory that holds the `revive.toml` this rule
writes and the JSON report revive answers with. `trap 'rm -rf "$work"'
EXIT` removes it, and the trap covers a clean run, a run with findings, a
run that declines an item and a broken run alike. A run that declines EVERY
path it is given exits before `mktemp -d` runs, so it makes no directory to
remove. Measured over a
`TMPDIR` of its own for each shape, with the count of entries taken before
the run and after it: the clean run, the run that declines a file revive
cannot parse, and the broken run over a path that holds no file each left
that count unchanged.
