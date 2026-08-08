---
name: complexity-typescript
description: TypeScript and JavaScript functions stay under the complexity and length gates — checked by eslint, not by prompt.
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
supersedes:
  - cognitive-complexity
  - function-length
tool:
  scope: files
  run: |
    modules="$(cd "$(dirname "$(readlink -f "$(command -v eslint)")")/../.." && pwd -P)"
    config="$(mktemp -d)/eslint.config.cjs"
    cat > "$config" <<'ESLINT_CONFIG'
    const tseslint = require("typescript-eslint");
    const sonarjs = require("eslint-plugin-sonarjs");
    module.exports = [
      {
        files: ["**/*.{js,jsx,mjs,cjs,ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
        plugins: { sonarjs },
        rules: {
          "sonarjs/cognitive-complexity": ["warn", 15],
          "max-lines-per-function": ["warn", {
            max: 250,
            skipBlankLines: true,
            skipComments: true
          }]
        }
      }
    ];
    ESLINT_CONFIG
    NODE_PATH="$modules" eslint --no-config-lookup --config "$config" --format json "$@" |
      jq -c '.[] | .filePath as $file | .messages[]
             | select(.ruleId == "sonarjs/cognitive-complexity"
                      or .ruleId == "max-lines-per-function")
             | {file: $file, line: .line, message: .message}'
  doctor:
    check_command: "which eslint jq"
    check_version_command: "eslint --version"
  install:
    commands:
      - "npm install -g eslint@10.8.0 typescript-eslint@8.66.0 typescript@5.9.3 eslint-plugin-sonarjs@4.2.0"
---

# Complexity and Length — TypeScript and JavaScript

`eslint` decides both gates in one run. Two rules carry it:

- `sonarjs/cognitive-complexity` — the published Sonar cognitive complexity
  score, the same metric the `complexity` probe computes.
- `max-lines-per-function` — a function that runs too long.

One run answers two prompt rules, so this rule names both in `supersedes`.

## The complexity gate is the probe's own metric

`eslint-plugin-sonarjs` implements Sonar rule S3776, which is the published
cognitive complexity algorithm. This is the one tool rule in the set whose
number the `cognitive-complexity` prompt rule would have produced: a `switch`
counts once, an `if`/`else if` chain stays flat, and each level of nesting adds
a penalty.

Two findings were hand-checked against the algorithm on this workspace and both
came out exact. `apps/kanban-app/ui/src/lib/format-date.ts`
`formatRelativeMagnitude` scores 18 — seven `if` statements at nesting 0, plus
five ternaries at nesting 1 that score 2 each, plus one ternary at nesting 0.
`progress-ring-display.tsx` scores 16, all of it nested ternaries.

The threshold stays at 15, the number the prompt rule states. The prompt rule's
second gate — condition-nesting depth 4 or more — has no eslint rule of its own,
so superseding drops it for TypeScript. That is the trade the tool rule makes.

## The length gate counts what the prompt rule counts

`skipBlankLines` and `skipComments` make `max-lines-per-function` count exactly
the lines the `function-length` prompt rule counts: "Exclude blank lines and
comment-only lines". Measured on a probe of 260 code lines carrying 52
comment-only lines and 52 blank lines, eslint counts 264 — the code lines plus
the signature line and the closing brace.

`max-lines-per-function` reads every function shape, an arrow function included,
which is what the prompt rule asks for: "Methods, closures, lambdas, standalone
functions".

## What the gates report on this workspace

Over the 444 `.ts` and `.tsx` files under `apps/`, the complexity gate reports
33 findings and the length gate 37.

The tool does not reproduce the prompt rule's test carve-out, and on this
workspace that is where nearly all of the length findings sit: 36 of the 37 are
`describe(...)` arrow callbacks in a `*.test.tsx` file, and 23 of the 33
complexity findings are in test files as well. A long `describe` block is a real
arrow function running over 250 lines, so the tool reports it. To exempt one,
write the inline suppression named below on it; prose does not exempt a tool
finding.

## Why the core length rule and not the sonarjs one

`eslint-plugin-sonarjs` also ships `sonarjs/max-lines-per-function`. It agrees
on the count — 264 on the same probe — but it takes its threshold as
`{maximum: N}`, and the bare-number form `["warn", 250]` that every other eslint
rule accepts silently does nothing: configured that way it reported zero
findings across all 444 files. The core rule states its counting in the two
options the prompt rule already names, so the core rule is the one this rule
runs.

## How the run is shaped

The script writes its own flat config to a temporary path and passes it with
`--config`. `--no-config-lookup` keeps eslint from reading the project's own
config, so the rule owns its whole invocation.

A config in a temporary directory cannot resolve a plugin by name, because node
searches for `node_modules` upward from the config file. The script therefore
sets `NODE_PATH` to the `node_modules` tree eslint itself lives in, which it
reads from the eslint command on the path: the command is a symbolic link to
`<node_modules>/eslint/bin/eslint.js`, so two directories above that link's
target is the tree. That names the same tree for a global install and for a
project-local one, and it stays correct when more than one `npm` is on the
path — `npm root -g` answers for the first `npm`, which need not be the one
that installed the eslint being run.

`typescript-eslint` supplies the parser. The default eslint parser cannot read
TypeScript syntax, so a `.ts` file without it is a parse error instead of a
finding. The install command pins `typescript` to 5.9.3 because
`typescript-eslint` accepts `typescript` below 6.1.0.

`check_command` names the two commands the pipe runs. `typescript-eslint` and
`eslint-plugin-sonarjs` are node modules rather than commands, so `which` cannot
name them; the fixture pair is what proves both resolved. Measured with
`NODE_PATH` pointed at an empty directory: eslint exits 2, writes its error to
stderr, and writes nothing to stdout, so the fail fixture produces no findings
and the doctor marks the rule unusable and falls it back to the prompt rules.

The scope is `files` because eslint reads the files it is given.

The `jq` filter selects the two owned rule ids and drops everything else eslint
emits on the same stream. That matters here beyond tidiness: a project file
carrying `eslint-disable` comments for plugins the temporary config does not
load turns each one into a "Definition for rule ... was not found" message —
155 of them on this workspace, plus 10 unused-directive messages. Selection here
is attribution, not exemption: to exempt one function, write
`// eslint-disable-next-line sonarjs/cognitive-complexity` — or the matching rule
name — above it in the code.
