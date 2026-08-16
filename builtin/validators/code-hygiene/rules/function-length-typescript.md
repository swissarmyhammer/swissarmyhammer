---
name: function-length-typescript
description: TypeScript and JavaScript functions stay under the length gate — checked by eslint, not by prompt.
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
supersedes: function-length
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    modules="$(cd "$(dirname "$(readlink -f "$(command -v eslint)")")/../.." && pwd -P)"
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    config="$work/eslint.config.cjs"
    cat > "$config" <<'ESLINT_CONFIG'
    const path = require("path");
    const tseslint = require("typescript-eslint");
    const { builtinRules } = require("eslint/use-at-your-own-risk");

    const FUNCTION_TYPES = new Set([
      "FunctionDeclaration",
      "FunctionExpression",
      "ArrowFunctionExpression",
    ]);
    const NAMED_MEMBER_TYPES = new Set([
      "MethodDefinition",
      "Property",
      "PropertyDefinition",
      "AccessorProperty",
    ]);
    const MOCHA_MODIFIER = new Set(["only", "skip"]);
    const MODERN_MODIFIER = new Set([
      "only", "skip", "todo", "failing", "fails", "fail", "fixme", "slow",
      "concurrent", "sequential", "serial", "parallel", "shuffle",
      "each", "for", "runIf", "skipIf",
    ]);
    // The framework function names are READ out of the tree eslint resolves.
    // Two files in that tree hold them. `eslint-plugin-sonarjs` exports the
    // set it calls the functions "whose callbacks define test structure rather
    // than business logic". `globals`, a declared dependency of that plugin,
    // holds every name Mocha and Jest define. No sonarjs RULE runs here.
    const SONARJS_STRUCTURE = "eslint-plugin-sonarjs/cjs/helpers/test-frameworks.js";
    // `globals` lists a framework's whole surface, so three of its own facts
    // take the names that open no test. A name that is itself a `globals`
    // environment is the framework namespace object, `mocha` and `jest`. A
    // name in `globals.chai` is an assertion entry, `expect`. `run` is Mocha's
    // delayed-start runner, which takes no callback at all, and it is the one
    // name no other file in the tree separates.
    const NOT_AN_OPENER = new Set(["run"]);
    // What the read gives on `globals` 17 and `eslint-plugin-sonarjs` 4.2. It
    // stands in only when the read throws, and the test
    // `the_shipped_typescript_function_length_config_reads_its_framework_names`
    // runs the shipped read and holds these two lists equal to its answer.
    const MIRROR_FRAMEWORK_FUNCTION = [
      "after", "afterAll", "afterEach", "before", "beforeAll", "beforeEach",
      "context", "describe", "fcontext", "fdescribe", "fit", "ftest", "it",
      "setup", "specify", "suite", "suiteSetup", "suiteTeardown", "teardown",
      "test", "xcontext", "xdescribe", "xit", "xspecify", "xtest",
    ];
    const MIRROR_MODERN_FUNCTION = [
      "afterAll", "afterEach", "beforeAll", "beforeEach", "describe", "fit",
      "it", "suite", "test", "xdescribe", "xit", "xtest",
    ];

    function readFrameworkFunctions() {
      try {
        const sonarjsDir = path.dirname(require.resolve("eslint-plugin-sonarjs"));
        const structure = require(SONARJS_STRUCTURE).TEST_FRAMEWORK_STRUCTURE_FUNCTIONS;
        const globals = require(require.resolve("globals", { paths: [sonarjsDir] }));
        const environment = new Set(Object.keys(globals));
        const assertion = new Set(Object.keys(globals.chai));
        const declared = [...Object.keys(globals.mocha), ...Object.keys(globals.jest)];
        const opener = declared.filter(
          (name) =>
            !environment.has(name) && !assertion.has(name) && !NOT_AN_OPENER.has(name),
        );
        const modern = new Set([
          ...Object.keys(globals.jest),
          ...Object.keys(globals.vitest),
        ]);
        const functions = [...new Set([...structure, ...opener])];
        return { functions, modern: functions.filter((name) => modern.has(name)) };
      } catch (error) {
        console.error(
          "function-length-typescript: the framework function names did not resolve (" +
            error.message + "); the mirror in the rule stands in",
        );
        return {
          functions: MIRROR_FRAMEWORK_FUNCTION,
          modern: MIRROR_MODERN_FUNCTION,
        };
      }
    }

    const FRAMEWORK_NAMES = readFrameworkFunctions();
    const MODERN_FUNCTION = new Set(FRAMEWORK_NAMES.modern);
    const FRAMEWORK_CALL = new Map(
      FRAMEWORK_NAMES.functions.map((name) => [
        name,
        {
          modifiers: MODERN_FUNCTION.has(name) ? MODERN_MODIFIER : MOCHA_MODIFIER,
          rooted: false,
        },
      ]),
    );
    // `globals` ships no Playwright environment, so Playwright's one opener
    // that no other framework spells is written. It is ROOTED: Playwright
    // writes it `test.step` and never bare, so a bare `step(...)` is measured.
    for (const name of ["step"]) {
      FRAMEWORK_CALL.set(name, { modifiers: MOCHA_MODIFIER, rooted: true });
    }
    const FRAMEWORK_ROOT = new Set(["test"]);

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

    function calleeNames(call) {
      const names = [];
      let callee = call.callee;
      for (;;) {
        if (callee.type === "CallExpression") callee = callee.callee;
        else if (callee.type === "TaggedTemplateExpression") callee = callee.tag;
        else if (callee.type === "MemberExpression") {
          if (callee.computed || callee.property.type !== "Identifier") return null;
          names.unshift(callee.property.name);
          callee = callee.object;
        } else if (callee.type === "Identifier") {
          names.unshift(callee.name);
          return names;
        } else return null;
      }
    }

    function isTestCall(call) {
      const names = calleeNames(call);
      if (!names) return false;
      for (let at = names.length - 1; at >= 0; at -= 1) {
        const shape = FRAMEWORK_CALL.get(names[at]);
        if (!shape) continue;
        const roots = names.slice(0, at);
        if (shape.rooted && roots.length === 0) return false;
        return (
          roots.every((name) => FRAMEWORK_ROOT.has(name)) &&
          names.slice(at + 1).every((name) => shape.modifiers.has(name))
        );
      }
      return false;
    }

    function readFunctions(ast) {
      const all = [];
      const exempt = new Set();
      const head = new Map();
      walk(ast, (node) => {
        if (FUNCTION_TYPES.has(node.type)) all.push(node);
        if (NAMED_MEMBER_TYPES.has(node.type) && node.value &&
            FUNCTION_TYPES.has(node.value.type)) {
          head.set(node.value, node.range[0]);
        }
        if (node.type !== "CallExpression") return;
        if (!isTestCall(node)) return;
        for (const argument of node.arguments) {
          if (FUNCTION_TYPES.has(argument.type)) exempt.add(argument);
        }
      });
      return { all, exempt, head };
    }

    function measuredAt(all, head, offset) {
      let found = null;
      let width = 0;
      for (const node of all) {
        const from = head.has(node) ? head.get(node) : node.range[0];
        if (offset < from || offset >= node.range[1]) continue;
        const span = node.range[1] - from;
        if (!found || span < width) {
          found = node;
          width = span;
        }
      }
      return found;
    }

    function exemptTestCallbacks(rule) {
      return {
        ...rule,
        create(context) {
          const sourceCode = context.sourceCode;
          const { all, exempt, head } = readFunctions(sourceCode.ast);
          const proxy = Object.create(context, {
            report: {
              value(descriptor) {
                const loc = descriptor.loc || (descriptor.node && descriptor.node.loc);
                const start = loc && (loc.start || loc);
                const at =
                  start && measuredAt(all, head, sourceCode.getIndexFromLoc(start));
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
              "max-lines-per-function": exemptTestCallbacks(
                builtinRules.get("max-lines-per-function"),
              ),
            },
          },
        },
        rules: {
          "code-hygiene/max-lines-per-function": ["warn", {
            max: 250,
            skipBlankLines: true,
            skipComments: true
          }]
        }
      }
    ];
    ESLINT_CONFIG
    status=0
    NODE_PATH="$modules" eslint --no-config-lookup --config "$config" --format json "$@" \
      > "$work/report.json" 2> "$work/eslint.err" || status=$?
    if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
      cat "$work/eslint.err" "$work/report.json" >&2
      printf 'function-length-typescript: eslint exited %s and judged no code\n' "$status" >&2
      exit 1
    fi
    filtered=0
    jq -r '.[] | .filePath as $file | .messages[] | select(.fatal)
           | "sah-diagnostic: eslint marked \($file) fatal and said: \(.message)"' "$work/report.json" \
      > "$work/unjudged.txt" || filtered=$?
    jq -c '.[] | .filePath as $file | .messages[]
           | select(.ruleId == "code-hygiene/max-lines-per-function")
           | {file: $file, line: .line, message: .message}' "$work/report.json" \
      > "$work/reported.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'function-length-typescript: jq could not read the eslint report\n' >&2
      exit 1
    fi
    while IFS= read -r line || [ -n "$line" ]; do
      printf 'sah-diagnostic: eslint declined an item and said: %s\n' "$line" >&2
    done < "$work/eslint.err"
    cat "$work/unjudged.txt" >&2
    cat "$work/reported.json"
  doctor:
    check_command: "which eslint jq cat mktemp"
    check_version_command: "eslint --version"
  install:
    commands:
      - "npm install -g eslint@10.8.0 typescript-eslint@8.66.0 typescript@5.9.3 eslint-plugin-sonarjs@4.2.0"
---

# Function Length — TypeScript and JavaScript

`eslint` decides the gate in one run. One rule carries it:
`max-lines-per-function`.

Every measurement below was made with eslint 10.8.0, typescript-eslint 8.66.0,
typescript 5.9.3 and eslint-plugin-sonarjs 4.2.0.

## The metric IS the prompt rule's own count

`skipBlankLines` and `skipComments` make `max-lines-per-function` count exactly
the lines the `function-length` prompt rule counts: "Exclude blank lines and
comment-only lines". Measured on a probe of 260 code lines carrying 52
comment-only lines and 52 blank lines, eslint counts 264 — the code lines plus
the signature line and the closing brace.

`max-lines-per-function` reads every function shape, an arrow function included,
which is what the prompt rule asks for: "Methods, closures, lambdas, standalone
functions".

So the gate carries the prompt rule's own number, 250, with no derivation. That
is the shape `function-length-dart`, `function-length-rust` and
`function-length-swift` each take. `function-length-go` and
`function-length-python` derive a number instead, because each of those counts
STATEMENTS rather than lines.

## The corpus the gate was measured over

Eight TypeScript repositories, cloned at HEAD on 2026-08-14:

| repository | commit | `.ts` and `.tsx` files |
|---|---|---|
| axios/axios | `e6824eec5fcf9da467a9792724396badc490c469` | 25 |
| nestjs/nest | `16a99fd748a969e5f98f4d20f109b6061b01552a` | 1817 |
| reduxjs/redux | `3084fc33bb23ab4cab4796172eafe342da4e73d9` | 53 |
| trpc/trpc | `6a70335e02fa1a8bc68e8d065b85687b0d7ffdea` | 964 |
| vitejs/vite | `dcf88bd2ad2b1a8845f9029587cc8c825e382d42` | 587 |
| vuejs/core | `a2b40db9a83b36ed9da3a16403cf8f040262d73f` | 489 |
| colinhacks/zod | `4e1720c80e65a6f2c8d1f9fc9da0ba3a1a4c9d86` | 424 |
| pmndrs/zustand | `2115efb9e270e73ad1d3472dfe0e0c7b8c6abcd4` | 34 |

4393 files. The corpus was run two times with the SHIPPED configuration at
`max: 1`, which makes eslint report every function and print that function's own
line count in its message — `Function 'buildProps' has too many lines (400).` —
one run with the test carve-out on and one with it off. 22506 functions came
back with their own number under the carve-out, and 39999 without it, so every
sweep below is arithmetic on the tool's own count rather than on a model of it.

| `max` | findings | the carve-out dropped | in a test path |
|---|---|---|---|
| 100 | 317 | 657 | 51 |
| 150 | 119 | 398 | 13 |
| 200 | 70 | 285 | 13 |
| 250 | 36 | 221 | 3 |
| 300 | 27 | 171 | 2 |
| 400 | 15 | 112 | 2 |

At the gate of 250 the corpus reports 36 functions and the carve-out drops 221.
Each of the 36 is a long procedural declaration: `vue` `baseCreateRenderer` at
1770 lines, `vue` `compileScript` at 794, `vite` `resolveConfig` at 658, `vue`
`createHydrationFunctions` at 615, `vite` `cssPostPlugin` at 598, `trpc`
`createRootHooks` at 576.

Three of the 36 stand in a test path, and all three are NAMED helpers, which is
the shape this set states stays listed: `vue` `testRender` at 1093, `vue`
`runSharedTests` at 660 and `trpc` `createAppRouter` at 277. A carve-out reading
the file name would have dropped those three; the mark at the definition keeps
them.

## The test carve-out the prompt rule states

`function-length` exempts "Functions explicitly marked as tests", and this set
names the mark: identify a test from its attribute or framework naming
convention at the **definition**, never from the file name. A complex helper
named `build_request` in a file called `foo_test.rs` is still a long function
and is still listed.

A `supersedes` claim the tool does not honour is a false claim, so this rule
states the carve-out itself, in the config it already owns. The config wraps the
eslint rule and drops a report whose measured function is an argument of a
test-framework call.

### What counts as a test-framework call

The wrapper reads the callee as a chain of names. It reads through a member,
through a chained call and through a tagged template, so `describe.only(...)`,
`it.each(rows)(...)`, ``it.each`table`(...)`` and `test.describe.serial(...)`
each give a chain. A computed property, such as `it[key]`, stops the read and
gives no chain.

One name in the chain must be a FRAMEWORK FUNCTION. Each name BEFORE the
framework function must be a FRAMEWORK ROOT, and each name AFTER it must be a
modifier that framework function accepts. The wrapper reads the chain from the
last name to the first, so `test.describe` takes `describe` as the framework
function and `test` as the root, and `test.each(rows)` takes `test` as the
framework function and `each` as a modifier. `test` is the one framework root.

### The framework function names are read, not written

A hand-written list of framework spellings was found wrong in one direction or
the other three review rounds running. The list is therefore READ, out of the
same `node_modules` tree the config already resolves through. Two files carry
it:

- `eslint-plugin-sonarjs/cjs/helpers/test-frameworks.js` exports
  `TEST_FRAMEWORK_STRUCTURE_FUNCTIONS`, which the plugin describes as the
  functions "whose callbacks define test structure rather than business logic":
  `describe`, `context`, `suite`, `it`, `test`, `specify`, `before`, `after`,
  `beforeEach`, `afterEach`, `beforeAll`, `afterAll`, `xdescribe`, `xcontext`,
  `xit`, `xtest`, `fdescribe`, `fcontext`, `fit` and `ftest`.
- `globals`, which `eslint-plugin-sonarjs` declares as a dependency, holds
  `globals.mocha` and `globals.jest`, every name each framework defines. Mocha's
  TDD interface stands only there: `setup`, `teardown`, `suiteSetup`,
  `suiteTeardown` and `xspecify`.

The two reads together give 25 framework functions.

**No sonarjs RULE runs in this rule, and the package is still required.** It is
the source of both reads. Measured: `globals` does not resolve from eslint's own
tree — `require.resolve("globals", { paths: [<eslint>] })` answers
`Cannot find module 'globals'` — and it arrives only with
`eslint-plugin-sonarjs`. And `fcontext`, `fdescribe` and `ftest` stand in the
sonarjs structure list and in neither `globals.mocha` nor `globals.jest`. So
dropping the package would break the carve-out that the corpus shows drops 221
findings at the gate.

`globals` lists a framework's whole surface, openers, hooks, namespace objects
and assertion entries alike, so three facts inside the same package take the
names that open no test. A name that is itself a `globals` environment is the
framework namespace object: `mocha` and `jest`. A name in `globals.chai` is an
assertion entry: `expect`. `run` is Mocha's delayed-start runner, which takes no
callback at all; it is the one name no other file in the tree separates, so the
config names it and this sentence states why.

Each framework function accepts the modifiers its own framework gives it, and
which framework that is comes from the same read. A name `globals.jest` or
`globals.vitest` declares accepts the full Jest, Vitest and Playwright set:
`only`, `skip`, `todo`, `failing`, `fails`, `fail`, `fixme`, `slow`,
`concurrent`, `sequential`, `serial`, `parallel`, `shuffle`, `each`, `for`,
`runIf` and `skipIf`. A name only `globals.mocha` declares accepts the two Mocha
spells, `only` and `skip`. `context` therefore accepts `only` and `skip` and
nothing else, which is what keeps `context.each(rows)(fn)` measured.

`globals` ships no Playwright environment, so Playwright's one opener that no
other framework spells — `step` — is written. It is ROOTED: a rooted framework
function needs a framework root before it, and Playwright writes `test.step` and
never a bare `step`. A bare `step(...)` and a `step.skip(...)` are therefore
measured, which is right, because `step` is a usual name for a build step, a
wizard step and a saga step.

A `globals` release that adds an opener, or a sonarjs release that moves the
helper, is caught rather than silent. The config carries a written mirror of
what the read answers, and the acceptance test
`the_shipped_typescript_function_length_config_reads_its_framework_names` runs
the SHIPPED config under node, holds the mirror equal to the read, and fails
when the config falls back to that mirror instead of reading.

The root is what holds the Playwright spelling. Playwright puts its whole
surface on the `test` root: `test.describe`, `test.beforeEach`,
`test.afterEach`, `test.step`, `test.describe.serial`, `test.describe.parallel`,
`test.fixme` and `test.slow`. The segment after the root is therefore a
framework function, and not a modifier. A mark that accepts only a modifier
after the root refuses all eight, and each one carries the mark
`function-length` exempts, so the refusal MAKES findings the superseded rule
would never make.

The per-function modifier is what holds the other direction. `context` is a
test-framework name only in Mocha, and Mocha gives it `only` and `skip` and no
other modifier. `context` is also a usual name for a React context, a request
context and an `AsyncLocalStorage`, and `each`, `for` and `run` are usual method
names on such an object. A mark that accepts any pair of a test name and a
modifier name therefore exempts `context.each(rows)(fn)` and
`context.for(rows)(fn)`, which are not test-framework calls.

The two rules together make the mark read "this is a test-framework call". The
mark does not read "the root identifier has a test name", and it does not read
"a test name stands beside a modifier name".

Measured with the shipped config on a probe of 44 spellings, each callback
running 266 lines against the gate of 250: all 34 test spellings give no
finding, and all 10 other spellings give one finding each. The 34 hold the
Vitest, Jest and Mocha forms — `describe`, `it`, `test`, `suite`, `context`, the
four hooks, `describe.only`, `describe.skip`, `describe.each(rows)`,
`it.each(rows)`, ``it.each`table` ``, `it.concurrent.each(rows)`,
`test.runIf(true)`, `context.only` and `context.skip` — and the Playwright
forms — `test.describe`, `test.describe` with no title, the four `test` hooks,
`test.step`, `test.step.skip`, `test.describe.serial`,
`test.describe.parallel`, `test.describe.serial.only`, `test.fixme`,
`test.slow`, `test.fail`, `test.only` and `test.skip`. The 10 hold
`context.run`, `context.each(rows)`, `context.for(rows)`, `context.map`,
`harness.describe`, `runner.test`, a plain call and a named helper inside a
`describe` block.

A second probe of 15 spellings holds the read itself, on the same body. The 13
Mocha and Jest globals a written list had left out — `before`, `after`, `setup`,
`teardown`, `suiteSetup`, `suiteTeardown`, `specify`, `xdescribe`, `xcontext`,
`xit`, `xspecify`, `fit` and `xtest` — give no finding, and the bare `step(...)`
and `step.skip(...)` give one each.

A third probe of 40 adversarial spellings holds the rest. The 26 test spellings
it holds give no finding, the Playwright surface that opens no test gives one
each — `test.use`, `test.extend`, `test.describe.configure`, `test.info` and
`test.setTimeout` — and so do `describe.describe`, `describe.test`, `step.test`,
`pipeline.step`, `harness.it`, `it[key]` and `describe[key].only`. Four
spellings stay exempt that no framework writes: `test.test`, `test.context`,
`test.suite` and `test.it`. Each is rooted at `test`, which is itself the mark,
so no finding is dropped on code a person writes.

### Why the mark and not the file name

A config block that turns the rule off for `**/*.test.*` and `**/*.spec.*` was
measured and rejected. Over the corpus at the gate of 250, three of the 36
findings stand in a test path and each of the three is a named helper —
`testRender`, `runSharedTests`, `createAppRouter` — which is the case this set
names word for word as still listed. A file glob drops those three, and a
carve-out that drops findings the superseded rule makes is the same false claim
in the other direction.

Two more plans were open and both are worse. Dropping `function-length` from
`supersedes` gives the carve-out back to the prompt rule and pays the LLM calls
this rule exists to remove. Writing an inline suppression on each block states
the exemption once for each test file, and a person must write it again for
every new one.

### What the carve-out costs

The mark reads the call. It does not read the length, so a test-framework
callback is exempt however long it runs: a `describe(...)` callback of 900 lines
is not a finding. The prompt rule makes the same trade — it exempts a test
whatever its length — so the reach is the prompt rule's reach and not a wider
one. A named helper stays measured wherever it stands, a test file and a
`describe` block included.

## Why the core length rule and not the sonarjs one

`eslint-plugin-sonarjs` also ships `sonarjs/max-lines-per-function`. It agrees
on the count — 264 on the same probe — but it takes its threshold as
`{maximum: N}`, and the bare-number form `["warn", 250]` that every other eslint
rule accepts silently does nothing: configured that way it reported zero
findings across 444 files. The core rule states its counting in the two options
the prompt rule already names, so the core rule is the one this rule runs.

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
path — `npm root -g` answers for the first `npm`, which need not be the one that
installed the eslint being run.

`typescript-eslint` supplies the parser. The default eslint parser cannot read
TypeScript syntax, so a `.ts` file without it is a parse error instead of a
finding. The install command pins `typescript` to 5.9.3 because
`typescript-eslint` accepts `typescript` below 6.1.0.

`check_command` names each command the script runs: `eslint`, `jq`, the `cat`
that writes the configuration and forwards each answer, and the `mktemp` that
makes the directory the configuration stands in.
`typescript-eslint` and `eslint-plugin-sonarjs` are node modules rather than
commands, so `which` cannot name them; the fixture pair is what proves both
resolved. Measured with `NODE_PATH` pointed at an empty directory: eslint exits
2, writes its error to stderr, and writes nothing to stdout, so the fail fixture
produces no findings and the doctor marks the rule unusable and falls it back to
the prompt rule.

The scope is `files` because eslint reads the files it is given.

The config wraps the rule instead of reading its report afterward. A wrapper
reads `context.sourceCode.ast` one time and collects three things: every
function, every function a test-framework call holds as an argument, and the
offset each function's head starts at. It then hands the rule a context whose
`report` drops a report whose measured function is one of those arguments.
eslint has no option that states this, and reading the report afterward cannot
state it either: a line number alone does not say which function the tool
measured.

### Why the head offset, and not the function's own start

The rule reports at the head of the function it measures, but that head is not
always inside the function's own range. `getFunctionHeadLoc` starts the head at
the parent's start for a `Property`, a `MethodDefinition` and a
`PropertyDefinition` — that is, at the member's NAME, which stands before the
`FunctionExpression` the rule measures. A lookup that reads only the function
ranges therefore misses the method and climbs to the function around it. Inside
a `describe` block that function is the test callback, so the method comes back
exempt.

The wrapper closes that hole. It stores the member node's own start offset for
each function a member holds, and it measures each candidate's span from that
offset. The method's span is then the smallest one that holds the report, so the
method wins over the function around it.

Measured with the shipped config: a 266-line class method and a 266-line getter
inside a `describe` block give the length finding. Before this offset was
stored, both were silent while the same shapes at the top level reported.

The wrapper reads the core rule through
`require("eslint/use-at-your-own-risk")`, eslint's own access to its built-in
rules. That is not a new dependency. A wrapped rule takes the name of the plugin
the config declares, so the id is `code-hygiene/max-lines-per-function`.

An eslint release that moves that access point breaks the config. The fail
fixture then produces no findings, and the doctor marks the rule unusable and
falls it back to the prompt rule — the same safe end as a plugin that does not
resolve.

The filter selects the one owned rule id as the findings, and it drops each
message that belongs to another rule. That matters here beyond tidiness: a
project file carrying `eslint-disable` comments for plugins the temporary config
does not load turns each one into a "Definition for rule ... was not found"
message. Measured over one file carrying two such comments: eslint wrote
`Definition for rule '@typescript-eslint/no-explicit-any' was not found.` under
that rule id at severity 2, and `Unused eslint-disable directive (no problems
were reported from 'code-hygiene/max-lines-per-function').` under a null rule id
at severity 1, and it judged the file's function lengths beside both. The one
message the filter does NOT drop is the message eslint marks `fatal`, which is
the section "A file eslint could not judge" below.

Selection here is attribution, not exemption: to exempt one function, write
`// eslint-disable-next-line code-hygiene/max-lines-per-function` above it in
the code.

## The run answers for its own arguments

eslint reads the working directory when it takes no path, and the configuration
this rule writes matches `**/*.{js,jsx,mjs,cjs,ts,tsx}`, so a run with no file
reaches every such file under the workspace root at exit 0. The script counts
its arguments first, and a count of zero exits 0 with no finding.

Measured over two TypeScript files, each holding one function of 266 lines:

| what the script is given | findings |
|---|---|
| no argument, before the guard | 2 |
| no argument, after the guard | 0 |
| the two files | 2 |

## The temporary directory the configuration stands in

`mktemp -d` makes the directory the eslint configuration, the eslint report and
eslint's own stderr are written into, and `trap 'rm -rf "$work"' EXIT` removes
it. The trap covers each way the script leaves — a clean run, a finding, a
declined item and a broken run — and it leaves the exit status of the script
alone.

Measured with a `TMPDIR` of its own, over one file: the directory this script
makes is gone after the run, and the one entry left under `TMPDIR` is node's own
`node-compile-cache`, which node writes and this script never makes. A second
run leaves that same one entry, and so does a run the status gate broke.

## A file eslint could not judge

The script reads eslint's own status and its own report, because each one
carries a shape a pipe ending in `jq` reads as a clean file. Measured with
eslint 10.8.0 against the shipped command line, over `judged.ts`, which holds
one function of 302 lines against the gate of 250:

| the run | exit | the report | stderr |
|---|---|---|---|
| `judged.ts` alone | 0 | one `code-hygiene/max-lines-per-function` message | 0 bytes |
| a file that does not parse beside it | 1 | that message AND one `fatal` message | 0 bytes |
| a file whose bytes are not UTF-8 beside it | 0 | that message, and an empty message list for the file | 0 bytes |
| a file whose `eslint-disable` comments name rules the config does not load | 1 | that message AND two messages of other rules | 0 bytes |
| a path that holds no file | 2 | 0 bytes | `No files matching the pattern "absent.ts" were found.` |
| a file nobody may read | 2 | 0 bytes | `Error: EACCES: permission denied, open ...` |
| a directory that matches no file | 2 | 0 bytes | `No files matching the pattern "emptydir" were found.` |

0 and 1 are the two statuses a measured run answers with. eslint exits 1 for a
message of error severity, and this rule's own findings are warnings, so a run
that reports every finding it made can answer either number. Every other status
is a broken run: the script forwards eslint's own stderr and its own report,
names the status, and exits 1. Rows 5 to 7 are what that gate is for. Each of
the three wrote 0 bytes of report, so the earlier pipe ending in `jq` read a
file eslint never opened as a clean file.

### The mark eslint puts on what it could not read

eslint states what it could not read as a message carrying `fatal: true`. Read
over the whole `lib/` tree of eslint 10.8.0, three places write that field and
no other:

| where | what it marks | is the file linted? |
|---|---|---|
| `services/parser-service.js` | a source its parser refused | no |
| `services/processor-service.js` | a source a processor refused | no |
| `linter/linter.js`, through `file-report.js` `addFatal` | an inline `/* eslint ... */` comment it could not read | yes |

The shipped config declares no processor, so the second row cannot be reached
from this rule. The first and third rows were each driven. Measured over
`judged.ts` beside a file whose function body never closes: the report carried
the length message of `judged.ts` AND one message reading
`Parsing error: '}' expected.` with `ruleId: null`, `fatal: true` and
`severity: 2`, at exit 1 — so eslint judged the file it could read while
refusing the one it could not. Measured over `judged.ts` beside a file carrying
`/* eslint code-hygiene/max-lines-per-function: [ */` above a function of 302
lines: eslint marked the comment fatal AND reported the length of the function
under it.

So `fatal` names an item eslint declined, and the run stays sound either way.
The script therefore keeps the length messages as findings and writes each
`fatal` message under the marker `builtin/validators/README.md` states, at
exit 0, in eslint's own words:

    sah-diagnostic: eslint marked <repo>/unparsable.ts fatal and said: Parsing error: '}' expected.

A null rule id is NOT that mark. Measured on the same channel: the message
`Unused eslint-disable directive (no problems were reported from
'code-hygiene/max-lines-per-function').` carries a null rule id at severity 1,
and eslint judged that file. `fatal` is the one field that separates the two,
which is why the filter reads the field rather than a message text.

Measured with the shipped script over `judged.ts` and the file that does not
parse: one finding on stdout, one marked line on stderr, exit 0. The acceptance
test
`the_shipped_typescript_function_length_tool_rule_declines_a_file_it_cannot_parse`
holds both halves, and a run that lost either one fails it.

A file whose bytes are not UTF-8 needs no decline. node decodes such a byte as
`U+FFFD` rather than refusing the file, so eslint parses what it read. Measured
over `judged.ts` beside a file holding two bytes that open no UTF-8 sequence:
the length finding stood, the other file's message list was empty, and the run
exited 0.

### The stderr channel, which a sound run leaves empty

The script states each line eslint wrote to stderr under the same marker, whole,
with no head read or stripped. A sound run writes 0 bytes there: measured over
`judged.ts` alone, over `judged.ts` beside the file that does not parse, over
`judged.ts` beside the file whose bytes are not UTF-8, and over `judged.ts`
beside the file carrying the two `eslint-disable` comments, each run wrote 0
bytes to stderr. Every measured shape that DOES write there exits 2, which the
status gate reads first, so no measured decline reaches the marker on that
channel today.

The loop stands for the shape this rule has not met yet. A head written down
here would answer for the one shape it was written for and stay silent for every
other, which is the lesson `missing-docs-python` records for ruff's own stderr.

## The carve-outs the prompt rule states

`function-length` exempts four shapes: a test, generated code, a function that
is mostly configuration or data, and an initialization function that sets many
fields. The run reproduces the test, through the mark at the definition. The
author answers every other one with the `eslint-disable-next-line` comment
above.

- **Configuration and data** the run does NOT drop. A data line counts like a
  code line, so a 300-row table inside a function reports.
  `function-length-rust` and `function-length-swift` record the same gap for the
  same reason, and the answer is the same: move the data out of the function, or
  write the comment.
- **An initializer that sets many fields** the run does not drop either. Each
  assignment is one line, so an initializer of more than 250 field assignments
  reports.
- **Generated code** the run does not drop. eslint holds no generated-file
  heuristic, and JavaScript states no header convention one could read; a header
  test in the script would name the first lines of one generator and never a
  convention. Go states one, which is why `function-length-go` drops a generated
  file. A project that generates TypeScript keeps the generated tree out of the
  review with its own ignore list, which is where the README puts a file list
  the project owns.
