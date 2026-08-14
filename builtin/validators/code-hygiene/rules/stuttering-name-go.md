---
name: stuttering-name-go
description: Exported Go names do not repeat their package name — checked by revive, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' '[rule.exported]' > "$work/revive.toml"
    revive -config "$work/revive.toml" -formatter json "$@" > "$work/revive.json"
    unread="$(jq -r '(. // []) | map(select(.RuleName == "")) | length' "$work/revive.json")"
    if [ "$unread" -ne 0 ]; then
      jq -r '(. // [])[] | select(.RuleName == "") | .Failure' "$work/revive.json" >&2
      exit 1
    fi
    jq -c '(. // [])[] | select(.RuleName == "exported" and .Category == "naming")
           | {file: .Position.Start.Filename, line: .Position.Start.Line, message: .Failure}' "$work/revive.json"
  doctor:
    check_command: "which revive jq mktemp"
    check_version_command: "revive -version"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/mgechev/revive@v1.15.0'
---

# Stuttering Name — Go

`revive` reports an exported type or function whose name opens with the name of
its own package. A caller outside the package writes the word two times —
`staged.StagedType` — and the second one carries nothing. The `exported` rule
holds that check.

This rule supersedes no prompt rule, because no shipped prompt rule reads a Go
name. The naming rules that ship — `swift/naming-clarity`,
`swift/doc-parameter-naming` and `js-ts/naming-and-style` — read no `.go` file,
and `missing-docs` asks for a doc comment rather than for a name. So a machine
without `revive` gets no answer to this question rather than a worse one, which
is the same fallback `unused-dependencies-rust` takes.

The scope is `files` because revive reads the files it is given. It needs no
`go.mod` to lint a loose file.

Every measurement below was made on revive 1.15.0.

## Two rules split revive's `exported` rule between them

`exported` makes findings of two kinds, and revive writes which kind in the
CATEGORY beside the rule name. A documentation finding carries `comments`. A
repetitive name carries `naming`. Both carry the rule name `exported`.

`missing-docs-go` owns the `comments` half. It supersedes the `missing-docs`
prompt rule and no other, a repetitive name is not a missing doc comment, and
its config therefore states `disableStutteringCheck` so revive never makes the
finding it does not own. This rule owns the `naming` half, so its config states
no argument at all and its filter selects the category.

Selection in the pipe is attribution, not exemption. The `comments` findings
this run drops are not exempt: `missing-docs-go` reports every one of them. The
two rules together are revive's whole `exported` output, with no finding owned
two times and none dropped. Measured over one file holding a documented
repetitive type, a plain undocumented type, an undocumented repetitive type, an
undocumented repetitive constant, variable, function and method, a name equal
to the package name, a name whose next rune is lower case, a name whose next
rune is an underscore, and one unexported type:

| the config | `comments` | `naming` |
|---|---|---|
| `[rule.exported]`, no argument — this rule | 10 | 4 |
| `arguments = ["disableStutteringCheck"]` — `missing-docs-go` | 10 | 0 |

The acceptance test
`the_shipped_go_rules_that_run_revives_exported_rule_split_its_findings` drives
both SHIPPED scripts over one file and holds that split.

## The filter reads the category, and never the sentence

The message revive writes is `type name will be used as staged.StagedType by
other packages, and that stutters; consider calling this Type`. The
`sayRepetitiveInsteadOfStutters` argument writes `and that is repetitive` in
place of `and that stutters`, and it moves nothing else. Measured over the same
four findings with the argument set and unset: the sentence changes, the
`Category` field stays `naming`, and the count stays 4.

This rule states neither argument, so the shipped message is the first form.
The filter reads `Category` all the same, because a filter on the word alone
breaks the day a config states the argument, and a filter on the category
cannot.

The acceptance test holds each finding to the QUALIFIED NAME the message
carries — `fixtures.FixturesRecord` — which is the part of the sentence that
does not move under either argument.

## What the check reads

The rule compares the exported name against the package clause of the file the
name stands in. Measured, one declaration for each row, in `package staged`:

| the declaration | reported |
|---|---|
| `type StagedType struct{}` | yes |
| `func StagedBuild() {}` | yes |
| `type Staged_Thing struct{}` | yes |
| `type StagedType struct{}` with a doc comment | yes |
| `type Staged struct{}` | no |
| `type Stagedly struct{}` | no |
| `const StagedLimit = 1` | no |
| `var StagedVar = 1` | no |
| `func (s StagedType) StagedMethod() {}` | no |
| `type stagedPrivate struct{}` | no |

Four properties hold, and each row above states one:

- The comparison is case-insensitive on the package name and needs a NEW WORD
  under it — an upper-case rune or an underscore. `Stagedly` continues the same
  word, so it reports nothing.
- A name no longer than the package name reports nothing. `staged.Staged` reads
  the word one time.
- The kind is a type or a function. A constant, a variable and a method each
  report nothing. A method name is never written package-qualified, so revive
  reads no method name; `checkRepetitiveNames` is called for a `FuncDecl` with
  no receiver and for a `TypeSpec`, and nowhere else.
- An unexported name reports nothing, because no caller outside the package can
  write it. That carve-out is Go's own capitalization rule.

A doc comment is not a defence. The fourth row is the documented repetitive
type, and it reports, which is the whole reason `missing-docs-go` alone cannot
answer this question.

## The tool survey behind the choice

`exported` holds the repetitive-name check alone. The whole Go lint space was
read before this rule was written, and each candidate was RUN over the probe
file the table above measures.

| tool | what it reports on the probe | owns the repetition |
|---|---|---|
| revive `exported` | the 4 repetitive names | yes |
| revive `var-naming` | `don't use underscores in Go names; type Staged_Thing should be StagedThing` | no |
| revive `confusing-naming`, `confusing-results`, `epoch-naming`, `error-naming`, `import-shadowing`, `package-directory-mismatch`, `package-naming`, `receiver-naming`, `unexported-naming`, `use-any` | nothing | no |
| staticcheck `-checks all` | `ST1000` package comment, `ST1003` the same underscore, `U1000` the unexported type | no |
| golangci-lint `default: all` | revive's own findings, and `unused` on the unexported type | no |

The revive row above is the whole naming space of that tool: 12 revive rules
write the `naming` category, and the table names all 12. `staticcheck` names
more than `exported` does — `ST1003` reads an underscore and an initialism, and
`ST1006`, `ST1011`, `ST1012` and `ST1016` each read a name of their own — and
none of them reads a name against its package. The golangci-lint row is 115
linters enabled at once, which is every linter that release carries.

So one tool answers this question, and this rule runs it.

## The three carve-outs revive makes for itself

`Apply` returns before the walk for a file that is not importable, and the
linter skips a generated file before that, so the same three carve-outs cover
this check and the documentation check alike.

Measured over four files, each holding one repetitive type:

| the file | reported |
|---|---|
| `staged.go`, `package staged` | yes |
| `staged_test.go`, the same bytes | no |
| `staged.pb.go` under the `// Code generated ... DO NOT EDIT.` header | no |
| `cmd/probe/main.go`, `package main` | no |

Each row differs from the first one in one thing:

- The test carve-out is the FILE NAME. The first two files hold the same bytes.
- The generated carve-out is the HEADER LINE and never the file name. Measured:
  a second `.pb.go` file holding the same bytes without the header reports its
  name.
- The command carve-out is the PACKAGE CLAUSE. Measured: the same declaration
  under `package Main` reports `Main.MainType`, so one rune of case is the
  whole difference.

The acceptance test
`the_shipped_go_stuttering_name_tool_rule_reads_neither_a_generated_a_test_nor_a_command_file`
holds the four positions.

## A run cannot answer zero for a broken tool

revive states a failure in two ways, and this script answers both, the way
`missing-docs-go` answers them.

- **revive exits nonzero** for a file that is not there, and for a config it
  cannot parse. The script writes revive's report to a file rather than into a
  pipe, and `set -e` makes revive's own failure the exit status of the script.
- **revive exits 0 for a Go file it cannot PARSE.** It states the failure with
  an empty `RuleName`, under the `validity` category. The category filter of
  this rule drops that record twice over, so the file would read as clean. The
  script counts the failures that belong to no rule, writes each one to stderr,
  and exits 1.

Measured over a file that does not parse beside a clean file: revive exits 0
and writes one record carrying `invalid file broken.go: broken.go:3:14:
expected ')', found '{'`, with `RuleName` empty and `Category` `validity`. The
acceptance test
`the_shipped_go_stuttering_name_tool_rule_breaks_on_a_file_it_cannot_parse`
holds the script to exiting 1 with that text on stderr.

`-formatter json` prints `null`, not an empty array, for a file with no
findings, so each `jq` filter starts with `(. // [])`.

## How to exempt one name

Write `//revive:disable-next-line:exported` above the declaration in the code.
Measured: the marker on the line above silences the finding, and
`//revive:disable:exported` inside a file silences the whole file. The passing
fixture carries the first form above a name that repeats the package name, so
the marker stays measured.

The first fix a finding asks for is still to rename. The message names the
rename it wants — `consider calling this Type` — and a caller then writes
`staged.Type`.

## The run answers for its own arguments

revive reads the package standing in the working directory when it takes no
path. A run with no file therefore answers for the workspace root package, at
exit 0, and it says nothing about a package deeper in the tree. The script
counts its arguments first, and a count of zero exits 0 with no finding.

Measured over two Go files, each holding one repetitive exported type, one at
the root and one three directories down, with no argument: 1 finding before the
guard, on the file at the root, and 0 after it. The same script over the two
files reports 2. The acceptance test
`the_shipped_go_stuttering_name_tool_rule_reads_only_the_files_it_is_given`
holds both halves.

## Why the script names no cache and no lock

`magic-numbers-go` and `function-length-go` each name a `GOLANGCI_LINT_CACHE`
directory of their own and each ask golangci-lint to serialize on its lock.
revive needs neither, and both halves are measured rather than assumed.

- **No lock.** `revive -h` states six flags — `-config`, `-exclude`,
  `-formatter`, `-max_open_files`, `-set_exit_status` and `-version` — and none
  of them is a lock or a cache. Measured over one workspace, eight runs of this
  script started together, two rounds: each of the eight reported all four
  findings in each round. golangci-lint stops the second instance with `Error:
  parallel golangci-lint is running` under the same probe.
- **No cache.** revive holds no answer between runs. Measured over a module of
  400 packages, each holding one repetitive exported type, and a copy of that
  module at a second path:

| the run | time | what it reported |
|---|---|---|
| the first directory, cold | 0.12 s | its own 400 paths |
| the first directory again | 0.11 s | its own 400 paths |
| the second directory, first run | 0.12 s | its own 400 paths |
| the second directory again | 0.11 s | its own 400 paths |

Row 3 is the whole answer: the same bytes at another path took the same time as
every other row, so there is no cold cost to pay and no cached answer to
inherit. The first directory was then removed and the second one still reported
its own 400 paths.

`dead-code-go` records the same verdict for `staticcheck` and reaches it the
other way: staticcheck DOES cache, and the workspace path is part of its key,
so a copy of the same bytes is a miss. revive caches nothing at all, so the
question does not arise. The acceptance tests
`the_shipped_go_stuttering_name_tool_rule_reports_while_other_runs_stand_together`
and `the_shipped_go_stuttering_name_tool_rule_reads_the_workspace_it_ran_in`
hold both halves.

## The temporary directory the configuration stands in

`mktemp -d` makes the directory that holds the `revive.toml` this rule writes
and the JSON report revive answers with. `trap 'rm -rf "$work"' EXIT` removes
it, and the trap covers the `exit 1` this script takes for a file revive cannot
parse. Measured over one file: one run raised the count of entries under
`TMPDIR` by 1 before the trap, and leaves that count unchanged after it.
