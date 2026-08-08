---
name: dead-code-typescript
description: TypeScript and JavaScript exports nothing imports — checked by ts-prune, not by prompt.
match:
  files:
    - "**/*.ts"
    - "**/*.tsx"
    - "**/*.js"
    - "**/*.jsx"
    - "**/*.mjs"
    - "**/*.cjs"
  project_types:
    - nodejs
supersedes: dead-code
tool:
  scope: workspace
  run: |
    find . -name node_modules -prune -o -name .git -prune -o -name 'tsconfig.json' -print |
      while IFS= read -r config; do
        dir="${config%/*}"
        prefix=""
        [ "$dir" = "." ] || prefix="${dir#./}/"
        (cd "$dir" && ts-prune -p tsconfig.json) |
          grep -v ' (used in module)$' |
          grep -v '\.d\.ts:' |
          sed -n "s#^\([^:]*\):\([0-9]*\) - \(.*\)\$#${prefix}\1:\2: unused export '\3'; nothing in the project imports it#p"
      done | sort -u
  doctor:
    check_command: "which ts-prune find grep sed sort"
    check_version_command: "npm list -g --depth 0 ts-prune | grep -o 'ts-prune@.*'"
  install:
    commands:
      - "npm install -g ts-prune@0.10.3"
---

# Dead Code — TypeScript

`ts-prune` reports every exported symbol no other module in the project
imports. That is a narrow, objective claim about the module graph, and it is the
whole of what this rule owns.

`knip` was the other candidate and is rejected; `VALIDATOR.md` records the
measurement. `ts-prune` was taken over it for two reasons: it has an inline
suppression the code can carry, and its claim is one sentence long.

## The staging contract

Write `// ts-prune-ignore-next` on the line above an export a later change will
import. Nothing else counts. A staged export with no marker is dead.

Measured on a probe project: an unannotated `export const` reports, and the same
export behind `// ts-prune-ignore-next` does not. Put the reason on the same line
after the marker, so the next reader can tell staged work from a leftover.

TypeScript has no `pub` and no `__all__` — every `export` is the module's
surface, and the module graph is the only thing that says whether the surface is
reached. Two shapes reach an export without importing it, and both need the
marker: a module a bundler aliases by path, and a function a framework registers
by name.

## What the pipe drops, and why

**`(used in module)`.** `ts-prune` marks an export that its own file uses. That
symbol is alive; only the `export` keyword is surplus. This rule reports dead
code, not surplus keywords, so those lines are dropped. Measured over
`apps/kanban-app/ui`: 178 raw lines, 103 of them carrying the marker.

**`.d.ts` files.** A declaration file is generated or ambient, and its
declarations exist to be read by the compiler rather than imported. Measured on
the same tree: 18 of the remaining 75 lines came from one generated file,
`src/lang-filter/parser.terms.d.ts`, which a lezer grammar build writes.

Both are attribution, not exemption. To exempt one export, write the marker in
the code.

## The measurement

Over this whole workspace — two `tsconfig.json` projects,
`apps/kanban-app/ui` and `apps/mirdan-app/ui` — the rule reports **58** findings
in **3.7 s**.

Ten were hand-checked. Seven are real: `BoardProgress`, an exported React
component nothing renders; `info`, `debug` and `trace`, three names of a
five-name re-export facade in `src/lib/log.ts` that no caller ever asked for;
the whole `src/components/fields/displays/index.ts` barrel, which every consumer
bypasses by importing the concrete module; and `clickInAct`, `getStrList`,
`RecentBoard` and `minimalTheme`, each defined once and referenced nowhere.

Three are reached by a name the import graph cannot see: the five exports of
`src/test/stubs/tauri-plugin-dialog.ts`, which `vite.config.ts` names as a
`resolve.alias` target, and the vitest browser commands in
`src/test/integration-commands.ts`, which a test calls as
`commands.createTestBoard(...)` after the config registers the module. Those are
the two shapes named above, and each takes a `// ts-prune-ignore-next`.

## How the run is shaped

The scope is `workspace` because "nothing imports it" is a whole-project
question. `ts-prune` reads a `tsconfig.json` to build the module graph, so the
script finds every `tsconfig.json` outside `node_modules`, runs the tool in that
project's own directory, and puts the project's path back on the front of each
finding. That is how a monorepo whose root carries no `tsconfig.json` still gets
checked. The engine keeps only the findings in the changed files.

The `tsconfig.json` a project already has is its build configuration, not lint
configuration: it is what makes the program a program. The rule reads it and
never writes one, and it never reads or writes any lint configuration.

`sort -u` collapses the duplicate a nested `tsconfig.json` produces when two
projects hold the same file.

Ending the pipe in `sed`, and the loop in `sort`, normalizes the exit status.
