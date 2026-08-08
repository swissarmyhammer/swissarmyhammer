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

## Complexity and length: three tool rules, and a probe that stays

`cognitive-complexity` and `function-length` are two prompt rules over one
concern — a function a reader cannot hold in their head. A linter decides both
for the languages that have one, so three tool rules supersede them:

- `complexity-rust` — one `cargo clippy` run over four lints:
  `excessive_nesting` at `6`, `too_many_lines` at `250`, `too_many_arguments`
  at `7`, and `type_complexity` at `250`. One run decides both gates, so this
  rule names both prompt rules.
- `complexity-python` — ruff `C901` at `max-complexity=15`. Supersedes
  `cognitive-complexity`.
- `function-length-python` — ruff `PLR0915` at `max-statements=180`, the
  statement count 250 code lines of Python measures out to. Supersedes
  `function-length`.

A tool measures its own way. Clippy's `excessive_nesting` counts lexical
nesting depth, `C901` counts McCabe decision points, and neither is the
published Sonar cognitive complexity the `complexity` probe computes, so the
numbers need not agree. Each tool rule's own file states what its tool measures
and what the threshold rests on.

The two languages split on the nesting gate. Rust keeps it: nesting depth is
the backbone of the Sonar cognitive metric, and `excessive_nesting` measures
exactly that. Python drops it, because ruff names no nesting rule. That is the
trade for Python — one number every reviewer gets the same, in place of two
numbers an agent reads off a probe.

The `complexity` probe stays. Every language without a healthy tool rule — and
every Rust or Python workspace whose tool doctor cannot find — keeps the probe
and the prompt rules. That is the designed fallback, not a gap.

## Dead code: two tool rules beside the prompt rule

The `dead-code` prompt rule owns the judgment half of dead code. Its carve-outs
— entry points, exported public API, and work-in-process scaffolding — need a
reader, and the `callers` probe gives that reader machine facts. No tool rule
supersedes it. A tool that replaced it would report staged work as dead.

Two tool rules run beside it, each deciding one question a tool can settle
alone:

- `unused-code-go` — `staticcheck -checks U1000`. An unexported Go item is the
  package's own business, so the whole set of possible callers is in the
  module.
- `unreachable-code-python` — `vulture --min-confidence 100`. A statement behind
  a jump that always runs can have no future consumer.

## Tools measured and rejected

Four candidates were measured and rejected. Each was installed and run before
the verdict.

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

### `cargo machete` — rejected

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

### `knip` — rejected

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

### `periphery` — rejected

Unused Swift declarations. Installed at 3.8.0 and run against a directory
holding a loose `.swift` file. It refused: "Failed to identify project in the
current directory. For Xcode projects use the '--project' option, and for SPM
projects change to the directory containing the Package.swift."

Periphery needs an Xcode project or an SPM package, and it builds that project
to index it. A review pass cannot pay a full build, and the fixture contract
gives a tool one loose file.
