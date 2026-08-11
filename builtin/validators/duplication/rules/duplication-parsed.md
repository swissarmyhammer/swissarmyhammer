---
name: duplication-parsed
description: A function, a method or a type that is nearly the whole of another one IS a duplicate — decided by the grammar, not by prompt.
match:
  files:
    - "**/*.ts"
    - "**/*.tsx"
    - "**/*.js"
    - "**/*.jsx"
    - "**/*.mjs"
    - "**/*.cjs"
    - "**/*.py"
    - "**/*.go"
    - "**/*.rs"
    - "**/*.java"
    - "**/*.c"
    - "**/*.h"
    - "**/*.cpp"
    - "**/*.cc"
    - "**/*.cxx"
    - "**/*.hpp"
    - "**/*.hh"
    - "**/*.hxx"
    - "**/*.rb"
    - "**/*.cs"
    - "**/*.php"
    - "**/*.f90"
    - "**/*.f95"
    - "**/*.f03"
    - "**/*.f08"
    - "**/*.f"
    - "**/*.for"
    - "**/*.swift"
    - "**/*.ex"
    - "**/*.exs"
    - "**/*.sh"
supersedes:
  - duplication
  - rust
  - swift
tool:
  scope: files
  run: |
    files=()
    for file in "$@"; do files+=(--files "$file"); done
    "$SAH_BIN" tool code_context duplication find "${files[@]}"
  doctor:
    check_command: '"$SAH_BIN" tool code_context duplication find --help'
    check_version_command: '"$SAH_BIN" --version'
    fix_hint: 'put the running sah binary on PATH, or set SAH_BIN to its path'
---

# Duplication — whole definitions, compared

The tool is `sah` itself. `sah tool code_context duplication find` parses each
file with the grammar the file itself is parsed with, walks the named
definitions the parse reports — every function, every method, every type — and
compares one definition against another.

The unit is the whole definition and nothing smaller. A definition that is
nearly the whole of an earlier definition IS a duplicate.

No model reads either copy. That is the whole point of this rule: how alike two
definitions are is a fact about the text, and a fact needs no judgment.

## The two normalizations

A definition is not compared as it is written. Each one is normalized first,
and what the normalization drops is exactly what the comparison is meant to see
past.

**A function or a method** normalizes its body. The first distinct identifier
becomes `v1`, the second `v2`, and each literal becomes a marker of its kind.
Two functions that differ only by their variable names, or only by one
constant, then normalize to the same stream. That is the case the prompt rule
always named as the real one — *"two blocks that differ only by a value are one
function with an argument"* — and the case a token matcher cannot see at all.

**A type** normalizes its members. Every declared name drops out and every
member type stays, in order. Two records whose members carry the same types in
the same order are one shape under two names.

The consequence is worth stating plainly: a record of six `String` fields
matches every other record of six `String` fields, whoever wrote it and for
whatever purpose. That is what the rule reports, on purpose. Where two such
records are genuinely unrelated, the marker comment below is the answer.

## The gate, and the measurement it came from

**Forty tokens, and ninety percent.** A definition of at least 40 normalized
tokens whose stream is at least 90 percent the same as an earlier definition's
is a finding.

The similarity is the length of the longest subsequence the two normalized
streams share, counted once on each side: `100 * 2 * shared / (left + right)`.
It is exact integer arithmetic on a parse, so a run over the same two
definitions always answers the same number. There is no similarity model, no
embedding and no second opinion.

Both numbers were measured over the 1183 tracked `.rs` files of this workspace,
not chosen. Each cell is the number of findings the rule reports at that pair
of gates:

| minimum tokens | 100% | 95% | 90% | 85% | 80% |
|---|---|---|---|---|---|
| 20 | 588 | 759 | 1035 | 1452 | 1991 |
| 30 | 365 | 451 | 585 | 806 | 1112 |
| **40** | 258 | 327 | **416** | 544 | 721 |
| 50 | 209 | 261 | 333 | 428 | 540 |
| 60 | 173 | 219 | 280 | 359 | 452 |
| 80 | 82 | 111 | 143 | 199 | 264 |
| 100 | 52 | 72 | 95 | 136 | 185 |

**Why forty and not twenty.** Under 40 tokens the report fills with shapes the
language forces on every author. `has_errors` and `has_warnings` in
`swissarmyhammer-doctor/src/runner.rs` are a 25-token pair of one-line
accessors over the same iterator. `ModelInfo`, `PerspectiveInfo` and `AddTag`
are three- and four-field records whose field types coincide because `String`
and `Option<String>` are what most records hold. At 40 and above every
sample read as a genuine copy.

**Why forty and not thirty.** The 30-token band still holds real findings —
`load_env_parsed` against `load_env_optional`, `Directory::xdg_data` against
`Directory::xdg_cache` — and it also holds five-field records that
coincide. The trade is stated rather than hidden: a missed finding leaves the
review where it was, and a wrong finding is a mandatory change to correct code.

**Why ninety and not one hundred.** An exact match after normalization answers
"identical once renamed", which is not the same question as "highly
alike". `builtin_yaml_sources` is copied between `swissarmyhammer-focus` and
`swissarmyhammer-commands` and scores 91, because one copy carries one extra
filter. A gate of 100 reports 258 of the 416 and drops that pair.

**Why ninety and not eighty.** Under 90 the records begin to collide by shape
alone. `ProjectSymbols`, a record of fifteen `String` fields, matches another
all-`String` record at 85. It is not a copy.

## The bounds on the work, and the measurement they came from

The comparison grows quickly in two directions. One pair fills a table of
`left * right` cells, so the cost of one pair grows with the square of the
definition. Two definitions of equal length always stay inside the gate's
reach, so the number of pairs grows with the square of how many definitions
share a length. No file of this workspace reaches either limit, but a generated
file and a table-driven file both have that shape.

**An equal stream is answered, not compared.** The definitions that normalize
to the same stream make one group. Two equal streams are 100 percent alike,
which is the highest answer the ratio gives, so each later definition of a
group reads its answer off the first one and makes no comparison at all. 258 of
the 416 findings are that case.

The measurement is one file that holds k definitions of one shape, each of 139
normalized tokens:

| k | before | after |
|---|---|---|
| 200 | 7.0 s | 0.1 s |
| 400 | 27.9 s | 0.2 s |
| 800 | 110.9 s | 0.5 s |
| 1600 | 431.9 s | 1.0 s |

Before, each doubling of k made the time four times larger. After, each
doubling makes it two times larger, which is the parse and nothing more. The
review that found the defect measured the same shape at 12.2 s, 46.4 s,
181.0 s and 728.9 s. The absolute numbers move with the size of the definition
the probe writes; the four-times-per-doubling shape is the same.

**A definition longer than 4096 normalized tokens is not compared.** One
uncapped pair near 28000 tokens costs 12.7 s alone, which is more than the
whole 1183-file sweep. The longest definition this workspace declares is 1744
normalized tokens, so the limit clears every real definition two times over and
holds one table under 17 million cells. A definition past the limit is silent:
the rule does not compare it and does not report it.

**A length band stops after 1024 shapes.** The widest band this workspace holds
is 748 shapes, so the limit clears it with room. A pair past the limit is
silent for the same reason.

**The report does not move.** The sweep gives the same 416 findings before the
bounds and after them, and the two reports agree line for line: a comparison of
the two shows no difference. Only the run time moves, from 10.5 s to 9.8 s.

The finding names both ends:

    src/writer.rs:88: fn `write_backup_row` is a near-duplicate of `write_row` at src/backup_writer.rs:41 (96 tokens, 94% alike)

A definition is reported once, against the closest of the definitions before
it, so a cluster of copies costs one finding for each copy rather than one for
each pair.

## What the change from a token window bought

This rule used to slide a 50-token window across each file's whole token stream
and hash each window. A window knows nothing about where a definition starts or
ends, so it reported the tail of one function matching the head of another, and
runs of boilerplate spanning two definitions.

| | window over tokens | whole definitions |
|---|---|---|
| findings | **945** | **416** |
| tracked `.rs` files | 1155 | 1183 |
| median finding | 67 tokens, 12 lines | 71 tokens |
| intra-file | 389 | 159 |
| run time | 6.7 s | 9.8 s |

The 416 are 395 functions, 13 structs and 8 enums or traits. The largest read
as real copies: `extract_verb_noun` is 347 tokens spelled identically in
`swissarmyhammer-skills/src/parse.rs` and `swissarmyhammer-agents/src/parse.rs`;
`generate_code_context_examples` and `generate_kanban_examples` are 378 tokens
at 92 percent; `build_clap_arg` in `swissarmyhammer-operations/src/cli_gen.rs`
is 98 percent of `new` in `apps/swissarmyhammer-cli/src/dynamic_cli.rs`.

A test attribute that carries arguments — `#[tokio::test(flavor =
"multi_thread")]` — was once read as naming nothing, so the
`#[tokio::test(...)]` functions of `swissarmyhammer-tools` were reported as
copies of each other. The rule reads the argument form now: an attribute names
a marker when its text IS the marker, or when the marker opens the argument
list. Measured over the 1189 tracked `.rs` files the workspace holds today, the
rule reports **406**, against the **414** the equality-only read reports on the
same tree. The eight findings between the two are the whole difference — seven
in `src/mcp/tools/review/tests.rs` and one in
`tests/integration/review_e2e.rs` — and no finding is added.

Those numbers are a whole-tree figure. The rule runs on the changed set, so a
review sees the copies the change carries and nothing else.

## Which engine, and why it is our own

The comparison is this workspace's own: definitions from its tree-sitter
roster, normalized as above, paired by a longest-common-subsequence ratio.

`cpd-core`, the core crate of the jscpd Rust engine, decided the earlier
version of this rule and has been removed. Its `detect_prepared` is a
Rabin-Karp rolling-hash detector over a token stream — it answers "where is a
run of N tokens spelled twice", which is the sliding window this rule no longer
wants, and it never sees a definition, so it cannot answer how alike two whole
definitions are. A sequence comparison answers that question and a rolling hash
cannot, so the dependency went with the window.

`jscpd` as a command stays rejected, and for the reason `VALIDATOR.md` records:
it scopes its input by path glob alone, and a path glob cannot take an inline
`#[cfg(test)] mod tests` out of a file that also holds production code. The
parse can.

## The exemptions are structural, and there are two

**A definition inside a test definition is exempt**, decided by the parse and
never by the file's name:

| Language | What marks the definition |
|---|---|
| Rust | `#[cfg(test)]`, `#[test]`, and any path ending in `::test` |
| Python | a `test_`/`Test` name, a `TestCase` base, a `@fixture` decorator |
| Go | a `Test`, `Benchmark`, `Example` or `Fuzz` name |
| JavaScript, TypeScript, TSX | a `describe`, `it`, `test`, `suite` or hook call |
| Java | `@Test`, `@ParameterizedTest`, `@BeforeEach` and their siblings |
| C# | `[Fact]`, `[Theory]`, `[Test]`, `[TestMethod]` and their siblings |
| C, C++ | a `TEST`, `TYPED_TEST` or `BOOST_AUTO_TEST_CASE` declarator, a `TEST_CASE` call |
| Swift | an `XCTestCase` base, a `test` name, `@Test`, `@Suite` |
| Ruby | a `describe`/`context`/`it` call, a `test_` name, a `TestCase` base |
| PHP | `#[Test]`, a `test` name, a `TestCase` base |
| Elixir | a `test`, `describe` or `setup` call |

Bash and Fortran get no test exemption, and that is a measured decision rather
than a gap: neither writes its tests beside the code they exercise — `bats` and
pFUnit each keep whole files of their own — so neither has a marker at a
definition to read, and a whole-file rule would have to read the path.

### What the exclusion is worth, measured

Over all 1189 tracked `.rs` files of this workspace: **406** findings. With the
Rust test markers taken out of the table and nothing else changed: **3216**.
The structural exclusion removes 2810 findings, 87.4% of the raw total.

That gap is the whole argument for parsing rather than globbing. A path glob
reaches none of those 2810, because they sit inside files that also hold
production code.

A duplicated test *helper* is still a finding, and on purpose: a helper is not
a test definition, so no structural marker exempts it, and the
`cognitive-complexity` rule already says the same thing in as many words.

**A definition a marker comment names is exempt.** Write

    // sah:allow duplication <reason>

on the line above the definition. The marker covers the next item, reaching
past a doc comment and past the item's own attributes. The text is what counts
and the delimiter never does, so `# sah:allow duplication <reason>` and
`/* sah:allow duplication <reason> */` say the same thing in the languages that
spell a comment that way.

Write the reason. The marker says the copy is deliberate; the reason says what
makes the two definitions fork rather than drift.

There is no third exemption. The carve-outs the `duplication`, `rust` and
`swift` prompt rules describe — a derive-style stub, a forwarding one-liner, a
conformance stub — are all far under forty tokens, so this rule never reaches
them. A carve-out that a reader would still argue for in prose is a marker
comment here, which is what turns the argument into a fact.

## Which definitions each language declares

| Language | Compared as a function | Compared as a type |
|---|---|---|
| Rust | `fn`, including an `impl` method | `struct`, `enum`, `trait` |
| TypeScript, TSX, JavaScript | function, class method | `interface`, `type` alias |
| Python | `def`, including a method | `class` |
| Go | `func`, method | `type` |
| Swift | `func`, including a method | `class`, `struct`, `enum`, `protocol` |
| Java | method, constructor | `class`, `interface`, `enum` |
| C# | method, constructor | `class`, `interface`, `struct`, `enum` |
| C, C++ | function | `struct`, `union`, `enum`, and C++ `class` |
| Ruby | `def`, singleton method | `class`, `module` |
| PHP | function, method | `class`, `interface`, `trait`, `enum` |
| Fortran | `function`, `subroutine` | — |
| Bash | function | — |
| Elixir | `def`, `defp`, `defmacro`, `defmacrop` | — |

An Elixir module and a Rust `mod` are left out on purpose: each is a namespace
holding the definitions below it, and neither is a unit worth comparing.

## How the run is shaped

The scope is `files` because a duplicate pair is a fact about the files handed
in. The engine hands the changed set to one process, so a definition pasted
into two brand-new files is caught in the same run as one repeated inside a
single file.

Two definitions are paired only when they parse to the same language, so a
`.rs` file and a `.py` file are never compared.

The script invokes `"$SAH_BIN"`, never a bare `sah`. The review engine exports
that variable to every tool-rule script, resolved as: an existing `SAH_BIN` in
the environment, then `std::env::current_exe()` when its file stem is `sah`,
then the bare name. So the rule runs the binary the engine is running inside
rather than whichever older copy sits first on `PATH`.

The op prints plain text, one `path:line: message` line per pair and nothing
else, so the pipe needs no `jq`. That shape is deliberate: `sah tool` renders a
JSON result as YAML, which the stdout contract cannot read.

The rule declares no install commands. The tool is sah, and a review is already
running inside it, so there is no package to pin and nothing to install. The
`doctor.fix_hint` names what a person does when `check_command` still fails.

## Which languages keep the prompt rule

Every language the grammar roster does not parse. The `match` above lists the
roster's extensions explicitly, and a test holds that list to the roster itself
so the two cannot drift. A file the roster does not claim reaches the
`duplication` prompt rule unchanged, together with the `duplicates` probe that
rule reads.
