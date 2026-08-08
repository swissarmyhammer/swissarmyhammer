---
name: code-hygiene
description: >-
  Flag hygiene defects in changed source code — commented-out code, overlong
  or overly complex functions, missing documentation on public APIs,
  hardcoded values that should be data, and dead code with no inbound
  callers.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - callers
  - complexity
---

# Code Hygiene

## Hardcoded values: two rules, one concern each

`data-driven` and `magic-numbers` split what one rule used to hold.

- `data-driven` owns the **shape**: a `match`/`switch` or `if`/`else if` chain over
  a known set whose arms differ only in constants is a table written out longhand.
  Reading the arms and deciding they differ only in constants is a judgment no
  tool makes, so no tool rule supersedes it.
- `magic-numbers` owns the **name**: a literal repeated across sites, or a shared
  configuration value, needs one constant. Four tool rules supersede it, each for
  the languages a linter can decide — `magic-numbers-python` (ruff `PLR2004`),
  `magic-numbers-typescript` (eslint `no-magic-numbers`), `magic-numbers-go`
  (`mnd`), and `magic-numbers-swift` (swiftlint `no_magic_numbers`).

Rust and Dart keep the `magic-numbers` prompt rule. No healthy Rust lint reports
an unnamed literal, and the Dart check needs a `custom_lint` package, which is a
dependency of the project under review rather than a tool the rule can install.

A tool reports by position and the prompt rule reports by repetition, so a tool
rule reports the one-off literal the prompt rule carves out. Each tool rule's own
file states the measurement behind its thresholds.

## Complexity and length: seven tool rules, and a probe that stays

`cognitive-complexity` and `function-length` are two prompt rules over one
concern — a function a reader cannot hold in their head. A linter decides both
for the languages that have one, so seven tool rules supersede them.

Three languages settle both gates in one run, so each of those rules names both
prompt rules:

- `complexity-rust` — one `cargo clippy` run over four lints:
  `excessive_nesting` at `6`, `too_many_lines` at `250`, `too_many_arguments`
  at `7`, and `type_complexity` at `250`.
- `complexity-typescript` — one `eslint` run over
  `sonarjs/cognitive-complexity` at `15` and `max-lines-per-function` at `250`
  with blank lines and comments skipped.
- `complexity-swift` — one `swiftlint` run over `cyclomatic_complexity` at `15`
  with `ignores_case_statements` on, and `function_body_length` at `250`.

Two languages name one tool for each gate, so each takes one rule for each:

- `complexity-python` — ruff `C901` at `max-complexity=15`, and
  `function-length-python` — ruff `PLR0915` at `max-statements=180`, the
  statement count 250 code lines of Python measures out to.
- `complexity-go` — `gocognit -over 15`, and `function-length-go` — `funlen`
  through golangci-lint at `lines: 250` with `ignore-comments` on.

A tool measures its own way, and only two of the five complexity gates are the
published Sonar cognitive complexity the `complexity` probe computes:
`sonarjs/cognitive-complexity` and `gocognit` are that algorithm, clippy's
`excessive_nesting` counts lexical nesting depth, `C901` counts McCabe decision
points, and swiftlint's `cyclomatic_complexity` counts decision points with
`switch` arms left out. So the numbers need not agree. Each tool rule's own file
states what its tool measures and what the threshold rests on.

The languages split on the nesting gate. Rust keeps it: nesting depth is the
backbone of the Sonar cognitive metric, and `excessive_nesting` measures exactly
that. TypeScript and Go keep it another way, because the Sonar metric charges a
function for its nesting inside the one score. Python and Swift drop it, because
ruff names no nesting rule and swiftlint's `nesting` rule measures nested type
and function declarations rather than nested conditions.

Dart takes no tool rule; see the rejection recorded below. It keeps the
`complexity` probe and both prompt rules.

The `complexity` probe stays. Dart, every other language, and every workspace
whose tool doctor cannot find keep the probe and the prompt rules. That is the
designed fallback, not a gap.

## Commented-out code: one tool rule, and the parse as the judge

`no-commented-code-parsed` supersedes the `no-commented-code` prompt rule for
eleven languages, and its tool is `sah` itself. The op it runs —
`sah tool code_context commented_code find` — extracts each comment block with
tree-sitter, strips the delimiters, and hands the text back to the grammar the
file itself is parsed with. Text that re-parses as two or more statements with
almost no error nodes IS code; text that does not is prose.

One rule reads eleven languages, which is what makes this rule unlike every
other one in the set: the verdict is a re-parse rather than a language-specific
linter, so Rust, Python, TypeScript, TSX, JavaScript, Go, Java, C, C++, C# and
Swift all take the same rule.

Five languages the grammar roster parses take no verdict, and each is a
measured decision. `bash`, `ruby` and `elixir` accept a paren-less call, so a
line of English parses as a command with arguments and a paragraph of prose
re-parses as clean code. `php` needs an opening tag a comment's text never
carries. `fortran` has no delimiter convention that separates documentation
from a disabled line. Those five, and every language the roster does not parse,
keep the prompt rule.

The exemption is structural on both halves. Documentation is excluded before
any gate runs — by the grammar's own doc-marker node in Rust, and by the
comment's own opening delimiter everywhere else — and a block of five lines or
fewer is under the gate. There is no ignore marker, because the thing the tool
reads is the grammar: move an example into a doc comment and the grammar
exempts it.

`ruff`'s `ERA001` was used to cross-check the Python fixtures rather than
shipped as a rule of its own. One finding has one owner, and the re-parse op
already covers Python.

## Dead code: six tool rules, and the prompt rule as the fallback

Dead code is objective. Six tool rules supersede the `dead-code` prompt rule,
one for each language a tool covers:

| Rule | Tool | Staging marker |
|---|---|---|
| `dead-code-rust` | `cargo check` `dead_code`, plus a `grep` orphan-module scan | `#[expect(dead_code, reason = "...")]` |
| `unused-code-go` | `staticcheck -checks U1000` | `//lint:ignore U1000 <reason>` |
| `dead-code-typescript` | `ts-prune` | `// ts-prune-ignore-next` |
| `dead-code-python` | `vulture` at its default confidence | `# noqa: V103` and its sibling codes |
| `dead-code-dart` | `dart analyze`, four unused diagnostics | `// ignore: unused_element` and its siblings |
| `dead-code-swift` | `periphery scan --retain-public` | `// periphery:ignore` |

The prompt rule stays as the fallback. It reads a language no tool rule covers,
and it reads any language whose tool `sah doctor` could not find. Its file now
says so, and it carries the same standard the tools carry.

### What made the question objective

Three of the prompt rule's four carve-outs were never judgments. A compiler
exempts an exported item, a test, and an entry point on its own, because it can
see which callers exist and which cannot: rustc never reports a reachable `pub`
item, `U1000` never reports an exported Go identifier, `dart analyze`'s
`unused_element` fires only on `_`-prefixed names, and `--retain-public` is the
same exemption for Swift. Python has no compiler, so `__all__` is the marker
vulture reads, and TypeScript has no marker at all — every `export` is surface,
and the module graph is the whole answer.

The fourth carve-out, work-in-process scaffolding, became an **annotation
contract**: staged code carries the language's own suppression marker with a
reason, or it is dead. The markers are in the table above. The
`builtin/validators/README.md` rules-for-tool-rules section states the general
form — an exemption a person would argue for in prose must become an inline
suppression the tool reads.

### Reversed and superseded decisions

- The `dead-code` **"do not supersede"** decision is reversed. It held that the
  carve-outs need a reader and that a tool replacing the prompt rule would
  report staged work as dead. The annotation contract answers the second half,
  and the compilers answer the first.
- The **`knip`** rejection is superseded by `dead-code-typescript`. `ts-prune`
  was taken instead: it carries an inline suppression, and its claim — an export
  no module imports — is narrower.
- The **`periphery`** rejection is superseded by `dead-code-swift`. The earlier
  verdict was made against a directory holding a loose `.swift` file, which
  periphery refused. The fixtures now carry a `Package.swift`, which is what the
  tool asks for, and `doctor.check_command` tests for one in the project so a
  workspace without an SPM package falls back to the prompt rule.
- The **`vulture` at default confidence** rejection is superseded by
  `dead-code-python`, and `unreachable-code-python` is folded into it so that
  one finding has one owner. The high false-positive rate the earlier verdict
  named is real and is answered where it belongs — `--ignore-names` and
  `--ignore-decorators` in the run script for framework patterns, `__all__` for
  the exported surface, and `# noqa: V1xx` for one name at a time.
- The **`cargo machete`** rejection is superseded by the `manifests` set's
  `unused-dependencies-rust` rule. The rejection was right that this set cannot
  host the tool, and wrong to stop there: a set-scope gap is closed by a set.
  `manifests` matches `Cargo.toml`, so a machete finding lands on a file the
  engine keeps, and the rule runs the default mode rather than the
  `--with-metadata` mode the misreporting half of the verdict measured.

## Tools measured and rejected

Five candidates were rejected. Four were installed and run before the verdict;
the fifth cannot be installed on the terms this set needs.

Three of the five verdicts no longer hold. `cargo machete`, `knip` and
`periphery` are marked superseded below, and the dead-code section above records
what replaced each one. A superseded verdict is kept rather than deleted, so a
reader can see what was measured, and when it stopped being the answer.

### `clippy::cognitive_complexity` — rejected

Clippy's own branch count, and the obvious candidate for the `complexity-rust`
rule. Rejected because it walks the macro-expanded AST.

This workspace builds `tracing` with the `log` feature. The log bridge expands
each call site into many branches, and clippy attributes those branches to the
caller. Measured on two probe crates with a byte-identical `src/lib.rs`, one
building `tracing` with `default-features = false` and one with the `log`
feature on: a flat, zero-branch function holding six `tracing` calls scores 7
without the feature and 43 with it.

The noise grows with how often a function logs, so no threshold separates it
from real branching, and the pipe cannot filter it — the finding carries a
function and a number, nothing more. At the gate of 15 it reports 460 findings
across this workspace, the mass of them sitting just over the gate.

`clippy::excessive_nesting` replaces it. On the same probe pair it reports
identical spans with and without the `log` feature, so the macro expansion
never reaches it.

### `cargo machete` — rejected, and the rejection is superseded by the `manifests` set

Unused Rust dependencies. Rejected for two independent reasons.

It cannot report. Every machete finding names a `Cargo.toml`, and this set
matches `@file_groups/source_code`, which declares no manifest pattern. A rule's
`match` narrows its set's and never widens it, and a `workspace`-scope run keeps
only the findings whose path is a matched changed file. A `Cargo.toml` finding
is therefore dropped on every path through the engine.

It also misreports. In `--with-metadata`, its accurate mode, it reports
`tauri-build` unused for `kanban-app` and for `mirdan-app`. Both build scripts
call `tauri_build::build()`. Its default mode misses that dependency kind by
scanning source text for the crate name, which cannot see a renamed or
feature-gated use either.

Unused dependencies are a manifest question, not a source-code one. A validator
set that matches manifests could host this tool; this set cannot.

That last sentence is what happened. The `manifests` set matches `Cargo.toml`,
and its `unused-dependencies-rust` rule hosts the tool. The misreporting half of
the verdict was answered rather than argued away: it is a property of
`--with-metadata`, and the rule runs the default mode, where neither `kanban-app`
nor `mirdan-app` appears among the 141 findings on this workspace. The default
mode's own blind spot — a dependency named by no source because a feature turns
it on — is real, and it is what
`[package.metadata.cargo-machete] ignored` is for. The rule's own file records
both measurements.

### `knip` — rejected, and the rejection is superseded by `dead-code-typescript`

Unused JavaScript and TypeScript files and exports. Run zero-config against
`apps/kanban-app/ui` with a full dependency tree installed.

It reports findings that are not defects. It calls
`src/test/stubs/tauri-plugin-dialog.ts` an unused file, but `vite.config.ts`
names that exact path as a `resolve.alias` target. It calls `info`, `debug`, and
`trace` unused exports of `src/lib/log.ts`, which is a one-line re-export
facade. Most of its 61 unused-export findings are the exported surface the
`dead-code` rule carves out, and knip has no way to tell that surface from a
leftover.

It also cannot meet the fixture contract. Knip reads a project — `package.json`,
`tsconfig.json`, and an installed `node_modules` — never a loose file, and
"unused" is a whole-project question, so a fail fixture and a pass fixture in
one directory cannot be judged apart.

### `periphery` — rejected, and the rejection is superseded by `dead-code-swift`

Unused Swift declarations. Installed at 3.8.0 and run against a directory
holding a loose `.swift` file. It refused: "Failed to identify project in the
current directory. For Xcode projects use the '--project' option, and for SPM
projects change to the directory containing the Package.swift."

Periphery needs an Xcode project or an SPM package, and it builds that project
to index it. A review pass cannot pay a full build, and the fixture contract
gives a tool one loose file.

Both halves of that verdict were answered rather than argued away. The fixture
directory now carries a `Package.swift.tmpl` whose one target names `path: "."`
and lists the two Swift dead-code fixtures, so the tool gets the package it
asks for. And the build is not a full one: measured on `Alamofire` at HEAD,
`swift build --build-tests` takes 5 s warm and the scan itself 1 s, and the
build is the project's own incremental cargo-equivalent, not a clean rebuild.

### DCM — rejected, and this is why Dart has no complexity tool rule

Dart's complexity and length metrics. It is the only Dart tool that computes
them, and it is not one this set can ship, so Dart keeps the `complexity` probe
and both prompt rules. That is a recorded verdict, not an oversight.

`dart_code_metrics` on pub.dev is discontinued at 5.7.6 and declares no
replacement package; its homepage now points at `dcm.dev`, which is a commercial
product. The Free tier is one seat and stops at 50k analyzed lines of code, and
a CI/CD license key — what an unattended review run needs — starts at the Teams
plan.

Both halves of the tool-rule contract fail on that. `install.commands` must pin
a version, and there is no free pinnable package left to pin: the pub package is
discontinued and the live product installs against a license. And a rule whose
tool needs a purchased key cannot degrade to its prompt rule cleanly for the
projects that do not hold one — it would be missing on every machine but the
buyer's.
