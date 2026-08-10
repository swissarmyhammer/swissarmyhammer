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
    const { builtinRules } = require("eslint/use-at-your-own-risk");

    const FUNCTION_TYPES = new Set([
      "FunctionDeclaration",
      "FunctionExpression",
      "ArrowFunctionExpression",
    ]);
    const TEST_CALL =
      /^(describe|it|test|suite|context|beforeAll|beforeEach|afterAll|afterEach)$/;

    function walk(node, visit) {
      if (!node || typeof node.type !== "string") return;
      visit(node);
      for (const key of Object.keys(node)) {
        if (key === "parent") continue;
        const value = node[key];
        if (Array.isArray(value)) value.forEach((child) => walk(child, visit));
        else walk(value, visit);
      }
    }

    function rootCalleeName(call) {
      let callee = call.callee;
      while (callee.type === "CallExpression" || callee.type === "MemberExpression") {
        callee = callee.type === "CallExpression" ? callee.callee : callee.object;
      }
      return callee.type === "Identifier" ? callee.name : "";
    }

    function readFunctions(ast) {
      const all = [];
      const exempt = new Set();
      walk(ast, (node) => {
        if (FUNCTION_TYPES.has(node.type)) all.push(node);
        if (node.type !== "CallExpression") return;
        if (!TEST_CALL.test(rootCalleeName(node))) return;
        for (const argument of node.arguments) {
          if (FUNCTION_TYPES.has(argument.type)) exempt.add(argument);
        }
      });
      return { all, exempt };
    }

    function innermostAt(all, offset) {
      let found = null;
      for (const node of all) {
        if (offset < node.range[0] || offset >= node.range[1]) continue;
        const span = node.range[1] - node.range[0];
        if (!found || span < found.range[1] - found.range[0]) found = node;
      }
      return found;
    }

    function exemptTestCallbacks(rule) {
      return {
        ...rule,
        create(context) {
          const sourceCode = context.sourceCode;
          const { all, exempt } = readFunctions(sourceCode.ast);
          const proxy = Object.create(context, {
            report: {
              value(descriptor) {
                const loc = descriptor.loc || (descriptor.node && descriptor.node.loc);
                const start = loc && (loc.start || loc);
                const at = start && innermostAt(all, sourceCode.getIndexFromLoc(start));
                if (exempt.has(at)) return;
                context.report(descriptor);
              },
            },
          });
          return rule.create(proxy);
        },
      };
    }

    module.exports = [
      {
        files: ["**/*.{js,jsx,mjs,cjs,ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
        plugins: {
          "code-hygiene": {
            rules: {
              "cognitive-complexity": exemptTestCallbacks(
                sonarjs.rules["cognitive-complexity"],
              ),
              "max-lines-per-function": exemptTestCallbacks(
                builtinRules.get("max-lines-per-function"),
              ),
            },
          },
        },
        rules: {
          "code-hygiene/cognitive-complexity": ["warn", 15],
          "code-hygiene/max-lines-per-function": ["warn", {
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
             | select(.ruleId == "code-hygiene/cognitive-complexity"
                      or .ruleId == "code-hygiene/max-lines-per-function")
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

## The test carve-out both prompt rules state

Each prompt rule this rule supersedes exempts a test. `function-length` exempts
"Functions explicitly marked as tests". `cognitive-complexity` exempts "A
function the probe marks as a test", and it says how to read the mark:
"Identify a test from its attribute or framework naming convention at the
**definition**, never from the file name. A complex helper named
`build_request` in a file called `foo_test.rs` is still a complex function and
is still listed."

A `supersedes` claim the tool does not honour is a false claim, so this rule
states the carve-out itself, in the config it already owns. The config wraps
each of the two eslint rules and drops a report whose function is the argument
of a test-framework call: `describe`, `it`, `test`, `suite`, `context`,
`beforeAll`, `beforeEach`, `afterAll` or `afterEach`. It reads the call through
a member and through a chained call, so `it.each(rows)("...", fn)` and
`describe.only("...", fn)` carry the mark as well.

### Why the mark and not the file name

The first plan was a second config block that turned the two rules off for
`**/*.test.*` and `**/*.spec.*`. Measurement rejected it. Of the 33 complexity
findings over the 444 `.ts` and `.tsx` files under `apps/`, 23 stand in a test
file, and 19 of those 23 are named helpers — `defaultInvoke`,
`defaultInvokeImpl`, `bootstrapInvokeImpl` — which is the case
`cognitive-complexity` names word for word as still listed. A file glob drops
those 19 findings, and a carve-out that drops findings the superseded rule
makes is the same false claim in the other direction.

Two more plans were open and both are worse. Dropping `function-length` from
`supersedes` gives the carve-out back to the prompt rule and pays the LLM calls
this rule exists to remove. Writing an inline suppression on each block states
the exemption once for each test file, and a person must write it again for
every new one.

The mark at the definition is one fact, stated one time, in the file that owns
the whole eslint invocation. It drops exactly the findings the two prompt rules
exempt.

### What the carve-out costs

The mark reads the call. It does not read the length or the score, so a
test-framework callback is exempt however long it runs and however high it
scores: a `describe(...)` callback of 900 lines is not a finding. The prompt
rule makes the same trade — it exempts a test whatever its length — so the
reach is the prompt rule's reach and not a wider one. A named helper stays
measured wherever it stands, a test file and a `describe` block included.

## What the gates report on this workspace

Over the 444 `.ts` and `.tsx` files under `apps/`, the two gates report 30
findings: 29 complexity and 1 length.

The same run without the carve-out reports 70 — 33 complexity and 37 length.
The carve-out drops 40 of them: 36 `describe(...)` callbacks over the line gate
and 4 `it(...)` callbacks over the complexity gate. It adds none, and it drops
nothing outside a test-framework callback. The 19 named helpers in test files
that score over the complexity gate all stay.

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

The config wraps the two rules instead of reading their reports afterward. A
wrapper reads `context.sourceCode.ast` one time, collects every function and
every function a test-framework call holds as an argument, and hands the rule a
context whose `report` drops a report the innermost function around it is one
of those arguments. Both rules report at the head of the function they measure
— the `function` keyword, or the `=>` token of an arrow — so the innermost
function around that point is the function under measurement. eslint has no
option that states this, and reading the reports afterward cannot state it
either: a line number alone does not say which function the tool measured.

The wrapper reads the core rule through
`require("eslint/use-at-your-own-risk")`, eslint's own access to its built-in
rules, and the sonarjs rule through the plugin's `rules` map. Neither is a new
dependency. A wrapped rule takes the name of the plugin the config declares, so
the two ids are `code-hygiene/cognitive-complexity` and
`code-hygiene/max-lines-per-function`.

An eslint release that moves either access point breaks the config. The fail
fixture then produces no findings, and the doctor marks the rule unusable and
falls it back to the prompt rules — the same safe end as a plugin that does not
resolve.

The `jq` filter selects the two owned rule ids and drops everything else eslint
emits on the same stream. That matters here beyond tidiness: a project file
carrying `eslint-disable` comments for plugins the temporary config does not
load turns each one into a "Definition for rule ... was not found" message —
155 of them on this workspace, plus 10 unused-directive messages. Selection here
is attribution, not exemption: to exempt one function, write
`// eslint-disable-next-line code-hygiene/cognitive-complexity` — or the matching
rule name — above it in the code.
