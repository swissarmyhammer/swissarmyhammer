---
name: complexity-go
description: Go functions stay under the complexity gate — checked by gocognit, not by prompt.
match:
  files:
    - "**/*.go"
  project_types:
    - go
supersedes: cognitive-complexity
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    remaining="$#"
    while [ "$remaining" -gt 0 ]; do
      remaining=$((remaining - 1))
      file="$1"
      shift
      if sed -n '/^package[[:space:]]/q;p' "$file" | grep -qE '^// Code generated .* DO NOT EDIT\.$'; then
        continue
      fi
      set -- "$@" "$file"
    done
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    gocognit -over 15 -json "$@" |
      jq -c '(. // [])[]
             | {file: .Pos.Filename, line: .Pos.Line,
                message: "cognitive complexity \(.Complexity) of func \(.FuncName) is over the gate of 15"}'
  doctor:
    check_command: "which gocognit go jq sed grep"
    check_version_command: "go version -m \"$(command -v gocognit)\" | awk '$1 == \"mod\" { print $2, $3 }'"
  install:
    commands:
      - 'mkdir -p "$HOME/.local/bin" && GOBIN="$HOME/.local/bin" go install github.com/uudashr/gocognit/cmd/gocognit@v1.2.1'
---

# Complexity — Go

`gocognit` reports every function whose cognitive complexity runs over the gate.
`-over N` is the one flag that names that check.

## This is the probe's own metric

`gocognit` implements the published Sonar cognitive complexity algorithm, which
is the same metric the `complexity` probe computes. Hand-checked on a probe
package: a `for` holding an `if` holding an `if`/`else if`/`else` scores 8 —
1 for the loop, 2 for the `if` one level in, 3 for the `if` two levels in, then
1 each for the `else if` and the `else`. That is the algorithm, term by term.

Length never leaks into the score. A flat 260-line function of `total += n`
scores 0.

The threshold stays at 15, the number the `cognitive-complexity` prompt rule
states. The prompt rule's second gate — condition-nesting depth 4 or more — has
no gocognit flag, so superseding drops it for Go. That is the trade the tool
rule makes.

Measured over the Go 1.26.5 standard library — 4350 files, `cmd/` and every
`testdata` directory left out — `-over 15` reports 2731 functions, against the
29580 that `-over 0` reports, which is every function that branches at all. The
distribution across the gate is smooth — 356 functions at 13, 282 at 14, 256 at
15, 232 at 16 — with no mass piled just over it, which is what a contaminated
metric looks like.

## How the run is shaped

The scope is `files` because `gocognit` reads the paths it is given, one
function at a time, and needs neither a `go.mod` nor a loaded package.

`-json` prints `null`, not an empty array, when nothing is over the gate, so the
pipe starts with `(. // [])[]`.

`-over 15` also exits 1 whenever it printed something. The pipe ends in `jq`,
which exits 0, so a run with findings is not read as a broken script.

`gocognit` carries no version flag. `go version -m` reads the module path and
version out of the installed binary instead, and the `awk` keeps the one `mod`
line, so `sah doctor` shows `github.com/uudashr/gocognit v1.2.1`. `go` is
therefore named in `check_command` beside `gocognit` and `jq`. The script also
runs `sed` and `grep` to read the head of each file it is given, so
`check_command` names those two as well.

## The annotation an author writes

`gocognit` reads one directive: `//gocognit:ignore`. Write it on its own line
in the doc comment of the function, and let no blank line stand between that
comment and the `func` line. The directive takes no text of its own, so the
reason stands on a comment line beside it:

    // TestTable walks each parser case.
    //gocognit:ignore
    func TestTable(t *testing.T) {

Measured with gocognit v1.2.1, over one function that scores 18 against the
gate of 15. Each of these spellings gives no finding: the directive alone; a
doc line above the directive; a doc line below the directive. Each of these
spellings gives one finding: `// gocognit:ignore` with a space in it;
`//gocognit:ignore // keep, parser table` with text after it;
`/*gocognit:ignore*/` as a block comment; `//Gocognit:ignore` with a capital
letter; and the directive with a blank line under it. `//nolint:gocognit` gives
one finding as well, because this rule runs the standalone binary and not
golangci-lint.

The first fix a finding asks for is still to split the function. The directive
is the second fix, and the comment line beside it states why.

## The two carve-outs the superseded prompt rule states

`cognitive-complexity` exempts a test function, and it exempts generated code.
The run reproduces the second carve-out. The first carve-out stays with the
author, as the directive above.

### Generated code, which the run drops

Go states one convention for a generated file: a line that matches
`^// Code generated .* DO NOT EDIT\.$` stands above the first text that is
neither a comment nor blank.

`gocognit` reads no such header. Its one file filter is `-ignore <regexp>`, and
that expression reads the PATH and never the content. Measured over three files,
one of them a `.pb.go` file that carries the header: `-ignore 'DO NOT EDIT'`
dropped none of the three, and `-ignore '\.pb\.go$'` dropped the file whose NAME
ends that way. A path expression can therefore name the file names of one
generator, and it can never name the convention.

So the script makes the test itself. For each file it is given, `sed` prints the
lines above the `package` clause and `grep` looks for the header line in them. A
file that carries the header is dropped before `gocognit` starts.

Measured over three files, each holding one function that scores 18: the script
without the test reported the ordinary file and the generated file; the script
with the test reports the ordinary file alone. The acceptance test
`the_shipped_go_complexity_tool_rule_skips_a_generated_file` holds both
positions. The two positions hold the same declarations, so the header is the
one difference between the file that reports and the file that stays silent.

The sibling `function-length-go` and `magic-numbers-go` run through
golangci-lint, which makes the same test for them. Measured over the same pair
of files: the default `linters.exclusions.generated` dropped the generated file,
and `generated: disable` reported it. The three Go rules of this set therefore
agree on what a generated file is.

An author cannot answer this carve-out with the directive. The generator writes
the file again and the directive goes away each time. That is why the run makes
the test and the author does not.

### A test function, which the run does not drop

The prompt rule exempts a function that is a test, and it names the DEFINITION
as the mark: "Identify a test from its attribute or framework naming convention
at the definition, never from the file name. A complex helper named
`build_request` in a file called `foo_test.rs` is still a complex function and
is still listed."

`gocognit` holds no flag that reads the name of a function, so it cannot make
that mark. Its `-test` flag is a boolean whose default is true, and it filters a
DIRECTORY WALK alone. This rule states `scope: files` and names each changed
file, so the flag reaches nothing. Measured over one directory holding one
ordinary file and one `_test.go` file, each with one function that scores 18:
the walk reported both files with `-test` and without it; `-test=false` reported
the ordinary file alone; and the same `-test=false` over the NAMED `_test.go`
path reported the test function again.

`-ignore '_test\.go$'` does drop a named `_test.go` path — measured, the same
file then reported nothing. The rule does not state that expression. It reads
the FILE NAME, which is the mark the prompt rule forbids, and it drops every
function of the file. Measured over one `_test.go` file that holds a `TestTable`
function and a `buildRequest` helper, each scoring 18: the run reports both, and
`-ignore '_test\.go$'` silences both. The prompt rule keeps the helper. An
expression that silences the helper trades a true finding for the carve-out,
which is the trade `magic-numbers-go` refuses for a shift operand.

So a complex test function REPORTS, and the author answers it. The first answer
is to move the table walk out of the test. The second answer is
`//gocognit:ignore` above the test function: measured, the same function then
reported nothing.

## The run answers for its own arguments

`gocognit` holds no default target. Given no path it writes 39 lines of
usage text to stderr and exits nonzero. The pipe ends in `jq`, which exits
0, so that refusal reached the engine as a clean tree. The script counts
its arguments first, and a count of zero exits 0 with no finding.

Measured over two Go files, each holding one function of cognitive
complexity 21, with no argument: 0 findings and exit 0 before the guard,
and the same after it. The same script over the two files reports 2. So
the guard makes the 0 an answer of the script's own, and it keeps the
usage text off stderr. The acceptance test
`the_shipped_go_complexity_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.

The generated-code test can empty the argument list of a run that started
with files in it. A change that touches a generated file alone leaves the
script no file to give the tool, and `gocognit` then writes its usage text
again. So the script counts its arguments a second time, under the test,
and a count of zero exits 0 with no finding. Measured over one generated
file alone: 0 findings and exit 0.
