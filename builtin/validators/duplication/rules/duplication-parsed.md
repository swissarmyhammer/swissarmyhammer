---
name: duplication-parsed
description: A token-identical block over the minimum window IS a duplicate — decided by the grammar, not by prompt.
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

# Duplication — the tokens decide

The tool is `sah` itself. `sah tool code_context duplication find` reads each
file with the grammar the file itself is parsed with, takes every code token,
and runs the jscpd Rust engine's rolling-hash detector over the stream. A run
of tokens spelled the same twice IS a duplicate.

No model reads either copy. That is the whole point of this rule: whether two
blocks are the same text is a fact about the text, and a fact needs no
judgment.

## The gate

**Fifty tokens.** A window of 50 tokens or more, token for token identical, in
one file or across two, is a finding. There is no similarity threshold and no
second opinion. Fifty tokens is about a dozen lines of ordinary code; below it
a match is the language's own grammar repeating — a `match` arm, an import
block, a struct literal — rather than a block someone pasted.

The finding names both ends:

    src/writer.rs:88: verbatim duplicate of src/backup_writer.rs:41 (15 lines / 96 tokens)

Comments are not tokens, so a copy with its comments rewritten is still a copy.
Identifiers are, so a copy with its variables renamed is a different block and
this rule says nothing about it. That case belongs to the `duplication` prompt
rule, which still runs for every language the grammar roster does not parse.

## Which engine, and why the tokens are ours

The detector is `cpd-core`, the core crate of the jscpd Rust engine
(MIT, `github.com/kucherenko/jscpd/tree/master/rust`). Its `detect_prepared`
entry point takes the token stream its caller supplies, which is what makes
this rule possible: the tokens come from this workspace's own tree-sitter
roster rather than from the engine's `cpd-tokenizer`, so the same parse that
finds the clone also decides which blocks are test code.

That is the difference this rule turns on. `jscpd` run as a command scopes its
input by path glob alone, and a path glob cannot take an inline
`#[cfg(test)] mod tests` out of a file that also holds production code. The
parse can.

## The exemptions are structural, and there are two

**A block inside a test definition is exempt**, decided by the parse and never
by the file's name:

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

Over all 1155 tracked `.rs` files of this workspace, in 6.7 s: **945** findings.
With the Rust test markers taken out of the table and nothing else changed:
**5077**. The structural exclusion removes 4132 findings, 81.4% of the raw
total — the same order as the 60.6% of jscpd's Rust clone instances that sit in
inline `#[cfg(test)]` modules, recorded in this set's `VALIDATOR.md`.

That gap is the whole argument for parsing rather than globbing. A path glob
reaches none of those 4132, because they sit inside files that also hold
production code.

The 945 that remain have a median of 67 tokens and 12 lines, and 389 of them
are intra-file. A read of the largest ones finds real copies: five CLI
`build.rs` files are the same file, and `extract_text` and `context_at` are
copied verbatim between `swissarmyhammer-tools`' inline review tests and its
`tests/integration/review_fixture.rs`. A duplicated test helper is a finding
here, and on purpose: a helper is not a test definition, so no structural
marker exempts it, and the `cognitive-complexity` rule already says the same
thing in as many words.

Those numbers are a whole-tree figure. The rule runs on the changed set, so a
review sees the copies the change carries and nothing else.

**A block a marker comment names is exempt.** Write

    // sah:allow duplication <reason>

on the line above the block. The marker covers the next item, reaching past a
doc comment and past the item's own attributes. The text is what counts and the
delimiter never does, so `# sah:allow duplication <reason>` and
`/* sah:allow duplication <reason> */` say the same thing in the languages that
spell a comment that way.

Write the reason. The marker says the copy is deliberate; the reason says what
makes the two blocks fork rather than drift.

There is no third exemption. The carve-outs the `duplication`, `rust` and
`swift` prompt rules describe — a derive-style stub, a forwarding one-liner, a
conformance stub — are all far under fifty tokens, so this rule never reaches
them. A carve-out that a reader would still argue for in prose is a marker
comment here, which is what turns the argument into a fact.

## How the run is shaped

The scope is `files` because a clone pair is a fact about the files handed in.
The engine hands the changed set to one process, so a block pasted into two
brand-new files is caught in the same run as a block repeated inside one.

Two blocks are paired only when they parse to the same language, so a `.rs`
file and a `.py` file are never compared.

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
