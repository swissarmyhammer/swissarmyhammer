---
name: code-hygiene
description: >-
  Flag hygiene defects in changed source code — commented-out code, overlong
  or overly complex functions, missing documentation on public APIs,
  hardcoded values that should be data, dead code with no inbound callers,
  and an exported Go name that repeats the name of its own package.
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
  configuration value, needs one constant. Five tool rules supersede it, each for
  the languages a linter can decide — `magic-numbers-python` (ruff `PLR2004`),
  `magic-numbers-typescript` (eslint `no-magic-numbers`), `magic-numbers-go`
  (`mnd`), `magic-numbers-swift` (swiftlint `no_magic_numbers`), and
  `magic-numbers-dart` (solid_lints `no_magic_number`).

Rust keeps the `magic-numbers` prompt rule; the survey below states why. Dart no
longer keeps it: the earlier verdict — that the Dart check needs a `custom_lint`
package, which is a dependency of the project under review — is **reversed**
below.

A tool reports by position and the prompt rule reports by repetition, so a tool
rule reports the one-off literal the prompt rule carves out. Each tool rule's own
file states the measurement behind its thresholds.

### The magic-number survey for Rust and Dart

Two languages had no rule, and neither had an obvious stock lint, so the whole
tool space was read for each before the verdict. This is the record.

**Rust — no usable tool, and the earlier claim is corrected.** The earlier claim
said "no healthy Rust lint reports an unnamed literal". The first half holds and
the second half does not: no CLIPPY lint reports one, and a dylint lint does.

- **clippy 0.1.97 (8bab26f4f6 2026-07-14)**, on rustc 1.97.1. `clippy-driver
  -Whelp` prints 1114 lines — every rustc lint, every clippy lint, and all nine
  groups, the opt-in `restriction` group included. Read whole, and filtered for
  `literal`, `magic`, `numeric`, `number` and `constant`: 69 lines match, and not
  one of them asks for a NAME. Each reads the literal's representation, its
  suffix, or its type — `decimal-literal-representation` (restriction) prefers
  hex, `default-numeric-fallback` (restriction) prefers a type suffix,
  `unreadable-literal` and `large-digit-groups` (pedantic) prefer underscores,
  and `inconsistent-digit-grouping`, `unusual-byte-groupings`,
  `mixed-case-hex-literals`, `zero-prefixed-literal`, `separated-literal-suffix`,
  `unseparated-literal-suffix`, `mistyped-literal-suffixes`,
  `excessive-precision`, `lossy-float-literal` and `non-ascii-literal` are the
  rest of that same family.
- **dylint (cargo-dylint 6.0.3, published 2026-08-01)** ships
  `examples/supplementary/unnamed_constant`, whose own README opens "Checks for
  unnamed constants, aka magic numbers." So the lint EXISTS, and the sentence
  that said no Rust lint reports an unnamed literal was wrong as written.

  It was installed and RUN before this verdict, in a toolchain home of its own
  so that no machine was changed. What it reads is the reason it is not taken.
  Measured over a probe crate holding one literal of each position, with the
  lint at its default `threshold` of `10`:

  | Written | Reported |
  |---|---|
  | `age > 18` | yes |
  | `part * 100` | yes |
  | `with_capacity(3600)` | yes |
  | `return 42;` | yes |
  | `n * 3600` | yes |
  | `word << 8` | no |
  | `n * 4096` | no |
  | `with_capacity(65535)` | no |
  | `42` as a tail expression | no |
  | `n * 7` | no |
  | `const NAMED_LIMIT: u64 = 4096;` | no |
  | `let timeout = 3600;` | no |

  The last two are the carve-out, and they are right. The four above them are
  the defect. `unnamed_constant` passes a value at or under the threshold, and
  it passes any value whose bits form one run — so `4096`, `65535` and every
  other power of two and all-ones mask are silent wherever they stand, and
  those are the values a name helps most. It reports `100`, which the prompt
  rule carves out for percent, and no setting restores that: `threshold` is its
  only key, and raising it to `100` would silence `status == 42` too. It reads
  a literal only where the parent node is an expression, so `return 42;`
  reports and a bare `42` tail expression does not.

  Two more properties would each be a problem on their own. The lint is
  published in no form a rule can pin: it is an example crate inside a git
  repository, built from source at first use, and it is on crates.io under no
  name. And building it needs a SECOND toolchain —
  `examples/supplementary/rust-toolchain.toml` pins
  `channel = "nightly-2026-05-28"` with
  `components = ["llvm-tools-preview", "rustc-dev"]`, because the lint links
  against `rustc_private` through `clippy_utils`. Measured: that toolchain
  takes **2.4 GB** on disk, `cargo install cargo-dylint@6.0.3
  dylint-link@6.0.3` takes 22 s, and the first `cargo dylint` run builds
  `clippy_utils` and the lint before it reads anything. Every machine that
  reviewed Rust would pay all of it, and then rebuild the crate under review
  with a custom rustc driver in a target directory of its own. Every other rule
  in this set is a released binary that runs in seconds.
- Nothing else in the space reports one either. `cargo machete`, `cargo udeps`,
  `cargo geiger`, `cargo audit` and `cargo deny` each read manifests or unsafe
  code, and `rust-code-analysis` computes metrics.

**Dart — a usable tool, and the earlier claim is reversed.** The earlier claim
said the Dart check needs a `custom_lint` package, "which is a dependency of the
project under review rather than a tool the rule can install". The first half
holds and the conclusion does not: the plugin is a dependency of the PROBE
PACKAGE the rule writes, and the project under review never sees it. The
`missing-docs-dart` rule already builds such a package, so the mechanism was in
the set before this rule used it.

- **The Dart SDK linter, 3.11.0 (Flutter 3.41.2)**: 263 rules, enumerated from
  the SDK's published rule index. None reports an unnamed literal, and the word
  "magic" appears nowhere in the index. `use_named_constants` is the nearest
  name, and it reports a literal that an EXISTING named constant already
  equals — `EdgeInsets.all(0)` for `EdgeInsets.zero` — rather than one that
  needs a name.
- **`lints` 6.1.0** (`core.yaml` 34 rules, `recommended.yaml` 55) and
  **`flutter_lints` 6.0.0** (10 rules over `recommended`): each is a selection
  from the same 263, so neither adds one.
- **`dart_code_metrics` 5.7.6**, the discontinued package the DCM verdict below
  names, carries a `no-magic-number` rule and a `metrics` executable. It cannot
  be installed on a current toolchain: its pubspec states
  `sdk: '>=2.18.0 <3.0.0'`, so `dart pub global activate` cannot resolve it
  against Dart 3.11.0. That is a firmer disqualification than "discontinued".
- **`solid_lints` 0.3.3** (a `custom_lint` plugin) carries `no_magic_number`,
  and it is what `magic-numbers-dart` runs. Measured over `dart-lang/http` at
  `a9176ac`, 324 files: 683 findings in 13 s.
- **`dart_code_linter` 4.1.9**, the maintained fork of `dart_code_metrics`, was
  measured over the same corpus: 653 findings in 5 s. It was NOT taken. It
  reports default parameter values, which the prompt rule carves out, and it
  exempts every literal inside a variable declaration's initializer, so it
  misses real findings. The rule file states the whole comparison.
- **`dcm`** is the commercial product, and the DCM verdict below still rejects
  it.

Two silent-zero traps were found and answered inside `magic-numbers-dart`, and
each one belongs to `dart run custom_lint`, the command that rule runs:
`--root-folder` does not move where `dart run custom_lint` reads its
configuration, and a failed `dart pub get` would leave a clean-looking run. Both
are recorded in the rule file.

## Complexity and length: eight tool rules, and a probe that stays

`cognitive-complexity` and `function-length` are two prompt rules over one
concern — a function a reader cannot hold in their head. A linter decides both
for the languages that have one, so eight tool rules supersede them.

Three languages settle both gates in one run, so each of those rules names both
prompt rules:

- `complexity-rust` — one `cargo clippy` run over four lints:
  `excessive_nesting` at `6`, `too_many_lines` at `250`, `too_many_arguments`
  at `7`, and `type_complexity` at `250`.
- `complexity-typescript` — one `eslint` run over
  `sonarjs/cognitive-complexity` at `15` and `max-lines-per-function` at `250`
  with blank lines and comments skipped. The config wraps both rules to keep
  the test carve-out that the two prompt rules state.
 - `complexity-swift` — one `swiftlint` run over `cyclomatic_complexity` at `15`
   with `ignores_case_statements` on, and `function_body_length` and
   `closure_body_length` each at `250`.

Two languages name one tool for each gate, so each takes one rule for each:

- `complexity-python` — `complexipy --max-complexity-allowed 15`, and
  `function-length-python` — ruff `PLR0915` at `max-statements=180`, the
  statement count 250 code lines of Python measures out to.
- `complexity-go` — `gocognit -over 15`, and `function-length-go` — `funlen`
  through golangci-lint at `statements: 160`, the statement count 250 code lines
  of Go measures out to.

Dart takes the LENGTH gate alone. `function-length-dart` runs
`dart_code_linter` 4.2.0 at `--source-lines-of-code=250`, and no rule carries the
branching gate for Dart. The survey below states why.

A tool measures its own way, and three of the five complexity gates are the
published Sonar cognitive complexity the `complexity` probe computes:
`sonarjs/cognitive-complexity`, `gocognit` and `complexipy` are that algorithm,
clippy's `excessive_nesting` counts lexical nesting depth, and swiftlint's
`cyclomatic_complexity` counts decision points with `switch` arms left out. So
the numbers need not agree. Each tool rule's own file states what its tool
measures and what the threshold rests on.

`complexity-python` ran ruff `C901` before, which is McCabe cyclomatic
complexity. It was replaced because that metric reads no nesting: measured over
one function of six nested `if` blocks, `C901` scores 7 and complexipy scores
21, so the gate stayed silent on the shape the prompt rule exists for. The rule
file carries the whole comparison.

The languages split on the nesting gate. Rust keeps it: nesting depth is the
backbone of the Sonar cognitive metric, and `excessive_nesting` measures exactly
that. TypeScript, Go and Python keep it another way, because the Sonar metric
charges a function for its nesting inside the one score. Swift drops it, because
swiftlint's `nesting` rule measures nested type and function declarations rather
than nested conditions.

Dart drops the branching gate whole, and the survey below states the
measurement. That is the same shape of trade `complexity-swift` makes for
nesting, one gate larger.

The `complexity` probe stays. Dart, every other language, and every workspace
whose tool doctor cannot find keep the probe and the prompt rules. That is the
designed fallback, not a gap.

### The complexity and length survey for Dart

Dart was the last language with no gate of either kind, so the whole tool space
was read before the verdict. Every measurement was taken on Dart SDK 3.11.0 with
Flutter 3.41.2, over 3931 `.dart` files — `dart-lang/http` at `a9176ac`,
`dart-lang/shelf` at `fb3f931` and `flutter/packages` at `a3e763e` — carrying
63241 functions.

Nothing in the stock toolchain measures either concern, and no preset can add
one. **`dart analyze`** at SDK 3.11.0 has no metric layer at all; its nearest
lint, `lines_longer_than_80_chars`, reads a line's WIDTH. **`lints` 6.1.0**,
**`flutter_lints` 6.0.0**, **`very_good_analysis` 10.3.0**, **`lint` 2.8.0**,
**`altive_lints` 4.1.0** and **`pedantic_mono` 1.38.1** are each a selection
from those same SDK rules — the first four declare no dependency at all — so
none of them can carry a check the SDK does not hold.

Four tools DO compute the metrics, and one of the four is usable.

- **`dart_code_metrics` 5.7.6** (2023-07-16) is discontinued, and it pins
  `analyzer >=5.1.0 <5.14.0` against a current analyzer of 14.x.
- **`dcm`** is its commercial successor. The rejection below still holds.
- **`solid_lints` 0.3.3** carries `cyclomatic_complexity` and
  `function_lines_of_code`, and its complexity rule is BROKEN. Its `run`
  registers `addDeclaration` INSIDE the `addBlockFunctionBody` callback, so a
  body's listener is added part way through the AST walk and then fires once for
  every Declaration visited AFTER it, each time re-measuring the captured body.
  Measured over one file holding five functions of 16 `if` statements: the first
  reports 12 times, the second 10, the third 8, the fourth 6 and the fifth 4 —
  the count is the number of declarations that follow, which is a fact about
  file layout. The fatal row: a file whose ONLY declaration is one function of 20
  `if` statements reports NOTHING, and adding `int trailing(int x) => x;` after
  it makes the same function report. A dirty file reads as clean.
- **`dart_code_linter` 4.2.0** (2026-08-11, Bancolombia, MIT) is the maintained
  fork, on `analyzer >=10.0.0 <15.0.0`. It takes every threshold as a CLI flag,
  writes JSON, and reports each function once. It is what `function-length-dart`
  runs.

Nothing outside the Dart ecosystem reaches it either. **lizard** has no Dart
parser. **PMD** supports Dart for copy-paste detection alone and has no Dart
rule engine. **scc**, **tokei** and **cloc** count lines per FILE and resolve no
function boundary. **semgrep** matches patterns and aggregates no metric.
**SonarQube**'s Dart analyzer does compute cognitive complexity, and it needs a
server and `sonar-scanner`.

So one tool computes all three metrics. `source-lines-of-code` is taken, and the
other two are rejected on what they measure.

**`cyclomatic-complexity` — rejected.** `cognitive-complexity` exempts
"Configuration parsing with many options, where the score comes from a long flat
list of simple cases rather than from nesting", and Dart's dominant idioms ARE
that list: a `copyWith` of N optional parameters writes N `??` operators, an `==`
writes N `&&`, and a `lerp` writes N ternaries. Cyclomatic complexity charges one
for each. At the gate of 15 the corpus reports 356 findings outside test files,
and **188 of them stand at nesting level 2 or less** — `InputDecoration.copyWith`
scores 59 at nesting 1 and is 59 named parameters each defaulted with `??`,
`ThemeData.==` scores 85, `DatePickerThemeData.lerp` 87. The published Sonar
cognitive metric scores a sequence of `&&` once rather than once per operator, so
it rates these near zero. No threshold separates them: the flat shapes run to
149.

**`maximum-nesting-level` — rejected.** The metric reads a widget tree three
constructors deep as 1 and a collection literal four deep as 1, both correct, and
it raises the depth by one for EVERY closure body. Dart is closure-heavy. The
prompt rule's gate is CONDITION-nesting depth 4 or more; at that gate the corpus
reports 229 findings outside test files and only 98 of them have a condition
depth of 4 or more, so **131 of 229 come from closures rather than conditions**.
34 have condition depth 0 or 1 — `_TabScaffoldExampleState.build` reaches level 6
through nested builder callbacks with no condition at all.

Under this set's contract a tool finding is a requirement, so either gate would
make hundreds of suppressions mandatory on code the prompt rule calls correct.
That is the trade `complexity-swift` refused for `switch` arms and
`function-length-go` refused for test paths.

## Commented-out code: no tool rule, and the prompt rule as the whole answer

`no-commented-code` is the whole of this gate. No shipped tool rule supersedes
it, so it reads every language the set matches.

`ruff`'s `ERA001` is the one language tool measured for the question, and it is
not shipped as a rule of its own. Measured at `ruff 0.14.5` with
`--isolated --no-cache --select ERA001`: it reports each commented-out line on
its own and it states no block-length option, so it cannot express the prompt
rule's "more than 5 lines" gate — a two-line commented-out snippet reports two
findings where the prompt rule stays silent. It also answers for Python alone.

## Dead code: six tool rules, and the prompt rule as the fallback

Dead code is objective. Six tool rules supersede the `dead-code` prompt rule,
one for each language a tool covers:

| Rule | Tool | Staging marker |
|---|---|---|
| `dead-code-rust` | `cargo check` `dead_code`, plus a `grep` orphan-module scan | `#[expect(dead_code, reason = "...")]` |
| `dead-code-go` | `staticcheck -checks U1000` | `//lint:ignore U1000 <reason>` |
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
vulture reads.

TypeScript names its surface one level up, in the PACKAGE rather than in the
module: `package.json` `main`, `exports`, `bin` and their siblings, and the
`tsconfig.json` `paths` mapping a repository writes under its own package name.
`dead-code-typescript` reads both and hands the modules they name to ts-prune's
own `--ignore`, which is `--retain-public` for TypeScript. Measured over three
published libraries, that carve-out takes `zod` from 1946 findings to 78,
`zustand` from 9 to 1 and `redux` from 14 to 6, and it moves this workspace's 58
by zero, because both of its TypeScript projects are private applications that
publish nothing. The rule file states each measurement.

The framework-registered entry point is the one carve-out no shipped rule
reproduces for TypeScript. ts-prune has no plugin and no configuration reader,
so a `vite.config.ts` alias target, a vitest browser command and a Next.js route
module each take the marker. `knip` answers that shape natively, and the
superseded verdict below records the measurement.

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
  no module imports — is narrower. Both reasons the rejection stated are wrong
  as written, and the re-measurement below says so. The inline suppression is
  the one property that still separates the two tools, and the swap is a card of
  its own.
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

## Naming: one tool rule, and no prompt rule behind it

`stuttering-name-go` reports an exported Go type or function whose name opens
with the name of its own package, because a caller outside the package then
writes the word two times — `staged.StagedType`.

It supersedes nothing, and it is the second rule of this set to do that after
the `manifests` set's `unused-dependencies-rust`. No shipped prompt rule reads a
Go NAME: the naming rules that ship are `swift/naming-clarity`,
`swift/doc-parameter-naming` and `js-ts/naming-and-style`, and none of the three
reads a `.go` file. A machine without `revive` therefore gets no answer to this
question rather than a worse one.

| Rule | Tool | Inline suppression |
|---|---|---|
| `stuttering-name-go` | `revive` `exported`, the `naming` category | `//revive:disable-next-line:exported` |

`missing-docs-go` runs the SAME revive rule and owns the other half of it. The
`exported` rule answers two kinds of finding under one rule name and tells them
apart by CATEGORY: a documentation finding carries `comments` and a repetitive
name carries `naming`. `missing-docs-go` states `disableStutteringCheck` and
owns the `comments` half; `stuttering-name-go` states no argument and selects
the `naming` half. The two together are revive's whole `exported` output with no
finding owned two times and none dropped, and the acceptance test
`the_shipped_go_rules_that_run_revives_exported_rule_split_its_findings` drives
both shipped scripts over one file and holds that split.

### The naming survey

The whole Go lint space was read before the rule was written, and each candidate
was RUN over one probe file. `revive`'s `exported` rule holds the check alone.

- **revive 1.15.0**: 12 rules write the `naming` category — `confusing-naming`,
  `confusing-results`, `epoch-naming`, `error-naming`, `exported`,
  `import-shadowing`, `package-directory-mismatch`, `package-naming`,
  `receiver-naming`, `unexported-naming`, `use-any` and `var-naming`. Over a
  probe holding a documented repetitive type, an undocumented repetitive type,
  an underscore name, a name equal to the package name, a name whose next rune
  is lower case, a repetitive constant, variable, function and method, and one
  unexported type: `exported` reports the four repetitive names, `var-naming`
  reports the UNDERSCORE alone, and the other ten are silent.
- **staticcheck 2025.1.1**, `-checks all` over the same probe: `ST1000`,
  `ST1003` on the same underscore, and `U1000` on the unexported type.
  staticcheck names more than `exported` does — `ST1003` reads an underscore and
  an initialism, and `ST1006`, `ST1011`, `ST1012` and `ST1016` each read a name
  of their own — and none of them reads a name against its package.
- **golangci-lint 2.12.2** with `default: all`, which is 115 linters: only
  `revive` reports the repetition, and `unused` reports the dead type.

`stuttering-name-go` drives revive DIRECTLY rather than through golangci-lint,
so it needs neither the `GOLANGCI_LINT_CACHE` directory nor the
`allow-serial-runners` key that `magic-numbers-go` and `function-length-go`
carry. Both halves are measured in the rule file: eight runs started together in
one workspace each reported every finding, and a module of 400 packages took the
same time cold and warm at two different paths.

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

**Both halves of that verdict were re-measured on 2026-08-14, and both are
wrong as written.** The second half is refuted by the shipped rule itself:
ts-prune also reads a project and never a loose file, and the fixture directory
now carries a `tsconfig.json.tmpl` that names the two TypeScript dead-code
fixtures, so the pair is judged the same way the Swift pair is. The first half
was measured against one private APPLICATION, which is the workspace where a
package declares no surface at all. Measured against three published libraries —
`zod` at `4e1720c`, `zustand` at `2115efb` and `redux` at `3084fc3` — knip's
entry-point resolution is the thing this set wants, and its plugins answer the
framework-registered entry point that `dead-code-typescript` still leaves to a
marker:

| workspace | `dead-code-typescript` | knip 6.32.0 |
|---|---|---|
| zod | 78 | 13 |
| zustand | 1 | 0 |
| redux | 6 | 2 |

`ts-prune` is nonetheless still the shipped tool, and it is now the WEAKER
choice on maintenance as well: `ts-prune` 0.10.3 was published on 2021-12-12,
its repository is archived, and its README names knip as the successor. The
swap is tracked as its own card, because two properties have to be answered
first. Knip has no line-comment suppression at all, so the staging contract
would move to a JSDoc tag and `/** @public */` states "public" rather than "a
consumer lands next". And knip exits 1 for a run that found issues and 2 for a
run it could not make, so the script has to tell those apart.

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

### DCM — rejected, and this is why Dart runs a fork rather than the product

Dart's complexity and length metrics. It is not a tool this set can ship.

The claim this section used to open with — that DCM is the ONLY Dart tool that
computes those metrics — is wrong as written. `dart_code_linter` 4.2.0 is a
maintained MIT fork of the same code base, it is free, it needs no key, and
`function-length-dart` runs it. The complexity survey above records that
measurement, and it records why the two metrics beside `source-lines-of-code`
are still rejected — on what they MEASURE, rather than on how they install.

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
