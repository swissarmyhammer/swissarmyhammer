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

## Dead code: which tools this set uses, and which it rejects

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

Three candidates were measured and rejected. Each was installed and run before
the verdict.

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
