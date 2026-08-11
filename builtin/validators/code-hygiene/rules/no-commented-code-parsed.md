---
name: no-commented-code-parsed
description: A comment block that re-parses as code IS commented-out code — decided by the grammar, not by prompt.
match:
  files:
    - "**/*.rs"
    - "**/*.py"
    - "**/*.ts"
    - "**/*.tsx"
    - "**/*.js"
    - "**/*.jsx"
    - "**/*.mjs"
    - "**/*.cjs"
    - "**/*.go"
    - "**/*.java"
    - "**/*.c"
    - "**/*.h"
    - "**/*.cpp"
    - "**/*.cc"
    - "**/*.cxx"
    - "**/*.hpp"
    - "**/*.hh"
    - "**/*.hxx"
    - "**/*.cs"
    - "**/*.swift"
supersedes: no-commented-code
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    files=()
    for file in "$@"; do files+=(--files "$file"); done
    "$SAH_BIN" tool code_context commented_code find "${files[@]}"
  doctor:
    check_command: '"$SAH_BIN" tool code_context commented_code find --help'
    check_version_command: '"$SAH_BIN" --version'
    fix_hint: 'put the running sah binary on PATH, or set SAH_BIN to its path'
---

# No Commented Code — the parse decides

The tool is `sah` itself. `sah tool code_context commented_code find` extracts
each comment block with tree-sitter, strips the comment delimiters, and hands
the text back to the grammar the file itself is parsed with. Text that
re-parses as several statements or items, with almost no error nodes, IS code.
Text that does not, is prose.

No model reads either one. That is the whole point of this rule: whether a
comment holds code is a fact about the text, and a fact needs no judgment.

## The three gates

A comment block is a finding when all three hold:

1. **It spans more than 5 lines.** The `no-commented-code` prompt rule this
   rule supersedes says "more than 5 lines of code that are commented out", and
   the number is unchanged.
2. **The re-parse yields 2 or more statements or items**, counted at any depth,
   so a commented-out function counts its body and not just itself. A statement
   holding nothing but a bare name does not count — tree-sitter-go reads the
   word `heading` as `(expression_statement (identifier))`, so a list of words
   would otherwise re-parse as a run of clean statements.
3. **Error nodes cover at most 0.07 of the re-parsed text.**

## Where 0.07 comes from

Measured, not guessed. Over this workspace and four external repositories —
`psf/requests`, `axios/axios`, `BurntSushi/ripgrep` and `gohugoio/hugo`, 2925
files in all — 1949 comment blocks clear the line gate. Every block whose error
ratio came out under 0.31 was read by hand, and three populations came out of
that reading:

| What the block is | Lowest ratio | Highest ratio |
|---|---|---|
| commented-out code | 0.000 | 0.035 |
| standardized metadata | 0.110 | 0.137 |
| prose | 0.173 | 0.999 |

The binding pair is 0.035 and 0.110. The highest ratio among real
commented-out code is hugo's `htmltemplate/exec_test.go`, six disabled calls
each carrying a `// TODO` tail. The lowest among the false positives is the
PEP 723 inline script metadata this workspace's own
`crates/ane-embedding/convert` scripts carry: every dependency line is a clean
Python assignment and only the `# ///` fences fail to parse. The gate sits
inside that gap — twice the code figure, two thirds of the metadata one.

A first cut at 0.15 admitted all three of those metadata headers. That is why
the number is measured rather than chosen.

## What the rule reports on real code

At 0.07: **0** findings across this workspace's 1610 measured files, **0**
across `psf/requests` and `axios/axios`, **1** on `ripgrep` and **2** on `hugo`.
All three were read by hand and all three are real — 22 disabled `println!`
lines in ripgrep's `crates/index/src/literal.rs`, a `/*if !c.skipTidy ...*/`
block in hugo's `modules/collect.go`, and the six disabled calls above.

## The exemption is structural, and it is the only one

**Put intentional example code in a doc comment, or keep the block at 5 lines
or fewer.** Those are the two exemptions, and neither is prose a reader has to
weigh.

Documentation is excluded before any gate runs. Where the grammar gives a doc
comment a node of its own the node kind is the test: Rust marks `///` and `//!`
with an `outer_doc_comment_marker` or `inner_doc_comment_marker` child, and it
is the only grammar in the roster that does. Where the grammar does not, the
test is the comment's own opening delimiter — `///`, `/**`, `//!`, `/*!` — which
is a token of the language and not a reading of the prose inside it. A Python
docstring is a string expression and never a comment node at all, so the
grammar excludes it before this rule looks.

A comment with live code to its left is an annotation on that line, never part
of a block. `gofmt` aligns a run of trailing comments into one column, so the
column is no test; the live code is.

There is no `no-commented-code:ignore` marker and there will not be one. A tool
rule's exemption must be something the tool reads, and here the tool reads the
grammar. Move the example into a doc comment and the grammar exempts it.

## What Python's cross-check said

`ruff`'s `ERA001` (`eradicate`) is the Python-only tool for this question. It
was run against the Python shapes rather than shipped as a rule of its own: one
finding has one owner, and the re-parse op already covers Python along with ten
other languages, so a second rule would report the same defect twice under two
names.

Run at `ruff 0.14.5` with `--isolated --no-cache --select ERA001`, the two
verdicts agree on every shape where both have an opinion:

| Shape | ERA001 | this rule |
|---|---|---|
| a commented-out five-line function | 3 findings | 1 finding |
| a six-line TODO written as prose | clean | clean |
| a code example inside a docstring | clean | clean |
| a PEP 723 `# ///` metadata header | clean | clean |
| a two-line commented-out snippet | 2 findings | clean |

The last row is the one difference, and it is the line gate rather than a
disagreement about the text: `ERA001` reports each commented-out line on its
own and has no block-length option, while this rule keeps the
`no-commented-code` prompt rule's "more than 5 lines".

Over this workspace's own Python, `ERA001` reports **1** finding — a two-line
comment in `crates/ane-embedding/convert` — and, like this rule, says nothing
about the three PEP 723 headers beside it.

## Which languages this rule covers, and which keep the prompt rule

Eleven: Rust, Python, TypeScript, TSX, JavaScript, Go, Java, C, C++, C# and
Swift. The `match` above lists their extensions explicitly, and a test holds
that list to the op's own coverage so the two cannot drift.

Five languages the grammar roster parses get no verdict, and each is a measured
decision rather than an omission:

- `bash`, `ruby` and `elixir` accept a paren-less call, so a line of English
  parses as a command with arguments and a paragraph of prose re-parses as
  clean code. No gate separates the two populations there.
- `php` needs an opening tag that a comment's text never carries.
- `fortran` has no delimiter convention that separates documentation from a
  disabled line.

Those five, and every language the roster does not parse at all, keep the
`no-commented-code` prompt rule. That is the designed fallback, not a gap.

## How the run is shaped

The scope is `files` because a comment and the code inside it live in one file.
Reading the file alone is the whole analysis.

The script invokes `"$SAH_BIN"`, never a bare `sah`. The review engine exports
that variable to every tool-rule script, resolved as: an existing `SAH_BIN` in
the environment, then `std::env::current_exe()` when its file stem is `sah`,
then the bare name. So the rule runs the binary the engine is running inside
rather than whichever older copy sits first on `PATH`.

The op prints plain text, one `path:line: message` line per block and nothing
else, so the pipe needs no `jq`. That shape is deliberate: `sah tool` renders a
JSON result as YAML, which the stdout contract cannot read.

The rule declares no install commands. The tool is sah, and a review is already
running inside it, so there is no package to pin and nothing to install. The
`doctor.fix_hint` names what a person does when `check_command` still fails.

## The run answers for its own arguments

The script builds a `--files` list out of `"$@"`. `files` is a required
parameter of the op, so an empty list is not a smaller run: `sah` answers
`missing required parameter 'files'` and exits 2, and the review then
reports a tool error over a change the rule had nothing to say about.

The script counts its arguments first, and a count of zero exits 0 with no
finding. Measured over two Rust files, each carrying one commented-out
block of 6 lines, with no argument: the script exited 2 before the guard,
and it reports no finding and exits 0 after it. The same script over the
two files reports 2. The acceptance test
`the_shipped_commented_code_tool_rule_reads_only_the_files_it_is_given`
holds the pair.
