---
name: missing-docs-typescript
description: Exported TypeScript and JavaScript items need JSDoc — checked by eslint, not by prompt.
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
supersedes: missing-docs
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    modules="$(cd "$(dirname "$(readlink -f "$(command -v eslint)")")/../.." && pwd -P)"
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    config="$work/eslint.config.cjs"
    cat > "$config" <<'ESLINT_CONFIG'
    const jsdoc = require("eslint-plugin-jsdoc");
    const tseslint = require("typescript-eslint");
    const body = " > :matches(FunctionExpression, TSEmptyBodyFunctionExpression)";
    const notAccessor = ':not([kind="get"]):not([kind="set"])';
    const notObvious =
      ':not([key.type="Identifier"][key.name=/^(toJSON|toLocaleString|toString|valueOf)$/])' +
      ':not([key.type="MemberExpression"][key.object.name="Symbol"])';
    const oneReturn = '[body.body.length=1]:has(BlockStatement > ReturnStatement)';
    const oneAssignment =
      '[body.body.length=1]:has(BlockStatement > ExpressionStatement > AssignmentExpression)';
    module.exports = [
      {
        files: ["**/*.{js,jsx,mjs,cjs,ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
        plugins: { jsdoc },
        rules: {
          "jsdoc/require-jsdoc": ["warn", {
            publicOnly: true,
            checkAllFunctionExpressions: false,
            require: {
              ArrowFunctionExpression: true,
              ClassDeclaration: true,
              ClassExpression: true,
              FunctionDeclaration: true,
              FunctionExpression: true,
              MethodDefinition: false
            },
            contexts: [
              "TSInterfaceDeclaration",
              "TSTypeAliasDeclaration",
              "TSEnumDeclaration",
              "MethodDefinition" + notAccessor + notObvious + body,
              'MethodDefinition[kind="get"] > :matches(TSEmptyBodyFunctionExpression, ' +
                "FunctionExpression:not(" + oneReturn + "))",
              'MethodDefinition[kind="set"] > :matches(TSEmptyBodyFunctionExpression, ' +
                "FunctionExpression:not(" + oneAssignment + "))"
            ]
          }]
        }
      }
    ];
    ESLINT_CONFIG
    NODE_PATH="$modules" eslint --no-config-lookup --config "$config" --format json "$@" |
      jq -c '.[] | .filePath as $file
             | select($file | endswith(".d.ts") | not)
             | .messages[]
             | select(.ruleId == "jsdoc/require-jsdoc")
             | {file: $file, line: .line, message: .message}'
  doctor:
    check_command: "which eslint jq mktemp"
    check_version_command: "eslint --version"
  install:
    commands:
      - "npm install -g eslint@10.8.0 eslint-plugin-jsdoc@63.3.3 typescript-eslint@8.66.0 typescript@5.9.3"
---

# Missing Documentation — TypeScript and JavaScript

`eslint` with `eslint-plugin-jsdoc` reports every exported function, class,
method, interface, type alias, and enum without a JSDoc comment. The
`publicOnly` option limits the rule to exported items, so an internal helper
needs no comment.

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

The scope is `files` because eslint reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one item, write
`// eslint-disable-next-line jsdoc/require-jsdoc` above it in the code.

Every measurement below was made on eslint 10.8.0 with
eslint-plugin-jsdoc 63.3.3 and typescript-eslint 8.66.0.

## The corpus

Six well known TypeScript repositories, each at the commit of its default
branch on the day of the measurement:

| repository | commit |
|---|---|
| axios/axios | `e6824eec5fcf9da467a9792724396badc490c469` |
| colinhacks/zod | `4e1720c80e65a6f2c8d1f9fc9da0ba3a1a4c9d86` |
| nestjs/nest | `16a99fd748a969e5f98f4d20f109b6061b01552a` |
| trpc/trpc | `6a70335e02fa1a8bc68e8d065b85687b0d7ffdea` |
| vitejs/vite | `dcf88bd2ad2b1a8845f9029587cc8c825e382d42` |
| vuejs/core | `a2b40db9a83b36ed9da3a16403cf8f040262d73f` |

4306 `.ts` and `.tsx` files, 58 of them `.d.ts`. Every file was handed to the
script's own eslint invocation, one configuration for each row:

| configuration | findings | of them in a `.d.ts` |
|---|---|---|
| every carve-out dropped | 9498 | 249 |
| the accessor carve-out alone | 9351 | 249 |
| the obvious-implementation carve-out alone | 9488 | 244 |
| both carve-outs | 9341 | 244 |
| both carve-outs, and the `.d.ts` filter | 9097 | 0 |

The three together take 401 findings off 9498, and they add none: the shipped
set is a subset of the earlier set, 157 positions removed by the two selectors
and 244 more by the filter, and 0 positions in the shipped set that the earlier
one did not hold.

A class member is 2940 of the 9498, and the two selectors read only those. By
kind of member, over the same corpus:

| kind of item | reported before | reported after |
|---|---|---|
| method, including a constructor | 2753 | 2753 |
| getter or setter | 177 | 30 |
| obvious implementation | 10 | 0 |
| every item that is not a class member | 6558 | 6558 |

## A method selector must end in the FUNCTION, not the method

`require: { MethodDefinition: true }` asks for a JSDoc comment on every method,
and it takes no argument that narrows the set. `contexts` takes an ESLint
selector, which does, so each method rule below stands in `contexts` and
`require.MethodDefinition` is off.

A selector that names the method itself reports almost nothing. The built-in
`MethodDefinition` visitor hands `checkJsDoc` the method's `value` — the
`FunctionExpression` — and never the `MethodDefinition`. `publicOnly` then runs
`exportParser.isUncommentedExport` on whatever node it was given, and that
function reads the two nodes differently: its `getExportAncestor` branch
answers false once the exported class carries a JSDoc block of its own, and its
`isExportByAncestor` branch accepts a `FunctionExpression` and refuses a
`MethodDefinition`.

Measured over one file holding one exported class of five methods and three
accessors, one time with a JSDoc block above the class and one time without:

| selector | class documented | class undocumented |
|---|---|---|
| `MethodDefinition[kind="method"]` | 0 | 5 |
| `MethodDefinition[kind="method"] > FunctionExpression` | 5 | 5 |

So every method selector ends in the function. That is the form the plugin's
own option description gives.

**A method with no body carries a different node.** An overload signature, an
optional `declared?(): void`, and a member of a `declare class` each hold a
`TSEmptyBodyFunctionExpression` rather than a `FunctionExpression`, and the
rule reported all three before. Each selector therefore ends in
`:matches(FunctionExpression, TSEmptyBodyFunctionExpression)`. Measured over
one file holding three classes and 17 members that are not abstract: a
`MethodDefinition` selector ending in the pair reports 17, and the same
selector ending in `FunctionExpression` alone reports 12. The five it loses are
the optional `declared?()`, two overload signatures, and the method and the
getter of a `declare class`.

An `abstract` method is a `TSAbstractMethodDefinition`, which is a node type of
its own. No selector here names it, and the built-in visitor never named it
either, so an abstract method needs no JSDoc comment under this rule and never
did.

## Simple accessors, which the selector carves out by BODY

The `missing-docs` prompt rule carves out "Simple getters/setters with
self-explanatory names". `simple` is the half a tool can decide, and the body
of the accessor is where it stands: an accessor that only moves a field holds
ONE statement, a `return` for a getter and an assignment for a setter.

The two selectors state that. A getter reports when its body is not a single
`return`, and a setter reports when its body is not a single assignment.
`self-explanatory` is left out, because no setting of any tool surveyed reads
a name for meaning.

Measured over the corpus: 177 accessors reported before, 30 after. The 147 that
go silent are each a single-statement accessor. The 30 that stay carry a second
statement, so the body does more than move the field, and the name alone cannot
say what.

**`checkGetters` and `checkSetters` are not the settings to reach for.** They
are first-class options of `require-jsdoc`, they default to `true`, and setting
either to `false` silences EVERY accessor of that kind whatever its body holds.
Measured over the corpus: `checkGetters: false, checkSetters: false` reports
9321 findings, 30 fewer than the selectors report, and those 30 are exactly the
accessors that hold more than one statement. The carve-out asks for a SIMPLE
accessor, and the option has no form for "simple". This is the verdict
`missing-docs-python` reaches for ruff's `ignore-decorators` and
`missing-docs-go` reaches for revive's `disableChecksOnMethods`, by the same
measurement.

An accessor with NO body — a `get value(): number;` inside a `declare class` —
reports. The carve-out holds where the body SHOWS the accessor only moves a
field, and a declaration shows no body at all.

The fail fixture carries a getter and a setter of two statements each, so the
edge of the carve-out stays measured from the reporting side, and the passing
fixture carries the single-statement pair, so it stays measured from the silent
side.

## Obvious implementations, which the selector carves out by NAME

The `missing-docs` prompt rule carves out "Obvious implementations (Display,
Debug, ToString, etc.)". `require-jsdoc` holds no option for it, so the
selector names the methods whose whole contract the language fixes, the way
revive names a fixed list for Go and ruff selects the magic-method code for
Python:

| the method | what fixes its contract |
|---|---|
| `toString` | `Object.prototype.toString` |
| `valueOf` | `Object.prototype.valueOf` |
| `toLocaleString` | `Object.prototype.toLocaleString` |
| `toJSON` | the `JSON.stringify` protocol |
| any `[Symbol.…]` method | the well-known symbol it is keyed by |

The first four are matched on an `Identifier` key, so a computed key of the
same text is not one of them. The fifth is matched on a `MemberExpression` key
whose object is `Symbol`, which is every well-known symbol at one stroke —
`Symbol.iterator`, `Symbol.asyncIterator`, `Symbol.toPrimitive`,
`Symbol.hasInstance`, `Symbol.dispose` and the rest — so a symbol added to the
language later needs no edit here.

Measured over the corpus: 10 findings, all of them one of these shapes. Five
stand in `axios/index.d.ts` — three `toJSON` overloads, a `toString` and a
`[Symbol.iterator]` — and five more are `toString` and `toJSON` methods in
`zod` and `nest`. The passing fixture holds one undocumented method of each of
the five rows, so a plugin release that reports one of them fails the fixture
pair.

`isExemptedImplementer` is the plugin's own neighbour of this carve-out, and it
is narrower: it exempts a method of a class that `implements` an interface, and
only when the matching interface member already carries a JSDoc comment. It
covers no method of a class that implements nothing, so it answers none of
these five rows. It stays on, at its default, beside the selector.

## Declaration files, which the pipe drops

The sibling `dead-code-typescript` drops a `.d.ts` file and states the reason:
"A declaration file is generated or ambient, and its declarations exist to be
read by the compiler rather than imported." The same reason holds for a doc
comment, and this rule follows the sibling rather than inventing a second
answer. The `jq` filter drops a finding whose file name ends in `.d.ts`.

Measured over the corpus: 244 of 9341 findings stand in a `.d.ts`, spread over
20 files. 111 of them stand in `axios/index.d.ts`, the hand-written public
declaration file of the package, and 58 in
`vite/packages/vite/src/types/ws.d.ts`, a copy of another package's
declarations vendored into the tree.

The filter reads `.d.ts` and no other name. `.d.mts` and `.d.cts` are the two
other declaration file names TypeScript writes, and neither can reach this
rule: `match.files` names `.mjs` and `.cjs` and never `.mts` or `.cts`.

This is attribution, not exemption. To exempt one item in an ordinary file,
write the inline suppression named above.

The doctor materializes one fixture as a loose file with no directory, so no
fixture pair can prove a carve-out the rule decides by name. The acceptance
test `the_shipped_typescript_missing_docs_tool_rule_reads_no_declaration_file`
stages the same undocumented interface at `src/staged.ts` and at
`src/staged.d.ts` and holds the run to reporting the first alone.

## Tests, which the rule carves out by the SHAPE of the callback

The `missing-docs` prompt rule carves out a function "explicitly marked as a
test by attribute or framework convention", and it names `it(...)` as the
TypeScript marker. That carve-out holds here, and the cause is worth stating
because it is not the cause it looks like.

The cause is not that `describe` and `it` are calls. It is that the rule reads
a function BOUND TO A NAME and never an anonymous argument. The
`ArrowFunctionExpression` visitor checks a function only when its parent is an
`AssignmentExpression`, an `ExportDefaultDeclaration`, a `VariableDeclarator`,
or a property whose value it is. The `FunctionExpression` visitor checks the
same four parents, plus every function expression when
`checkAllFunctionExpressions` is on. A callback handed to `it(...)` has none of
those parents.

Measured over one file holding an exported helper, an arrow callback inside
`it(...)`, and a `function` callback inside `it(...)`:

| configuration | exported helper | arrow callback | `function` callback |
|---|---|---|---|
| the shipped rule | reported | silent | silent |
| `publicOnly` off | reported | silent | silent |
| `checkAllFunctionExpressions: true` | reported | silent | silent |
| both together | reported | silent | reported |

So two independent settings hold the `function` form silent, and the arrow form
answers to no setting at all. The config now writes
`checkAllFunctionExpressions: false` rather than leaning on its default, so the
one setting that can turn the `function` form on is written down. The arrow
form has no setting to write down, so the passing fixture carries a `describe`
holding one callback of each form, and the fixture pair is what holds it.

The exported helper reporting is what the prompt rule asks for word for word:
"Identify test items from the structural marker on the item itself ... not from
the file name or path." The rule reads no path, so an exported helper in a test
file keeps its requirement.

## Private items, which `publicOnly` carves out

`publicOnly: true` reads an exported item and no other. Measured over one file:
a class that is not exported reports nothing, and neither does any member of
it. Inside an exported class, a `private` method, a `protected` method, a
`#private` method and a `private` getter each report nothing, while a `public`
method, a `static` method and the constructor each report.

## The run answers for its own arguments

eslint takes the working directory as its target when the command line
names no path, and the configuration this rule writes matches
`**/*.{js,jsx,mjs,cjs,ts,tsx}`. A run with no file therefore asks for a
JSDoc comment on every declaration under the workspace root, at exit 0.
The script counts its arguments first, and a count of zero exits 0 with no
finding.

Measured over two TypeScript files, each exporting one undocumented
function:

| what the script is given | findings |
|---|---|
| no argument, before the guard | 2 |
| no argument, after the guard | 0 |
| the two files | 2 |

The acceptance test in `shipped/missing_docs.rs` holds both halves: the run
with no argument, and the run over the two files.

## The temporary directory the configuration stands in

`mktemp -d` makes the directory the eslint configuration is written into,
and `trap 'rm -rf "$work"' EXIT` removes it. The exit status of the pipe
stays the exit status of the script. Measured over one file: one run
raised the count of entries under `TMPDIR` by 1 before the trap, and
leaves that count unchanged after it.
