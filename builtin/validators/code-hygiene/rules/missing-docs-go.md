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
    work="$(mktemp -d)"
    printf '%s\n' '[rule.exported]' '  arguments = ["disableStutteringCheck"]' > "$work/revive.toml"
    revive -config "$work/revive.toml" -formatter json "$@" > "$work/revive.json"
    unread="$(jq -r '(. // []) | map(select(.RuleName == "")) | length' "$work/revive.json")"
    if [ "$unread" -ne 0 ]; then
      jq -r '(. // [])[] | select(.RuleName == "") | .Failure' "$work/revive.json" >&2
      exit 1
    fi
    jq -c '(. // [])[] | select(.RuleName == "exported")
           | {file: .Position.Start.Filename, line: .Position.Start.Line, message: .Failure}' "$work/revive.json"
  doctor:
    check_command: "which revive jq"
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
same rule name a documentation finding carries, so a filter cannot tell the two
apart.

That finding is about a name. This rule supersedes the `missing-docs` prompt
rule and no other, and the `naming-consistency` prompt rule owns a name. A rule
that reported both would give one defect two owners.

`disableStutteringCheck` is the revive setting that turns the name check off.
Measured over two files that hold three stuttering names and two undocumented
items: the config without the setting reports 5 findings, and the config with
the setting reports the 2 documentation findings and no other.

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

## A run cannot answer zero for a broken tool

revive states a failure in two ways, and the earlier pipe read neither.

- **revive exits nonzero** for a file that is not there, and for a config it
  cannot parse. A pipeline takes the exit status of its LAST command, and that
  command was `jq`, so the run exited 0 with no output. The script now writes
  revive's report to a file rather than into a pipe, and `set -e` makes revive's
  own failure the exit status of the script.
- **revive exits 0 for a Go file it cannot PARSE.** It states the failure with
  an empty `RuleName`, under the `validity` category. A filter that selects the
  `exported` findings drops that record, so the file read as clean. The script
  counts the failures that belong to no rule, writes each one to stderr, and
  exits 1.

Measured over a file that does not parse and a clean file together: the earlier
pipe reported one finding and exited 0; the script reports no finding and exits
1, with `invalid file broken.go: ...` on stderr. The acceptance test
`the_shipped_go_missing_docs_tool_rule_breaks_on_a_file_it_cannot_parse` holds
that behaviour.

`-formatter json` prints `null`, not an empty array, for a file with no
findings, so each `jq` filter starts with `(. // [])`.

## How to exempt one item

Selection in the filter is attribution, not exemption: to exempt one item, write
`//revive:disable-next-line:exported` above it in the code. Measured: the marker
on the line above silences the finding, and `//revive:disable:exported` inside a
file silences the whole file.
