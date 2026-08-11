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
    module.exports = [
      {
        files: ["**/*.{js,jsx,mjs,cjs,ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
        plugins: { jsdoc },
        rules: {
          "jsdoc/require-jsdoc": ["warn", {
            publicOnly: true,
            require: {
              ArrowFunctionExpression: true,
              ClassDeclaration: true,
              ClassExpression: true,
              FunctionDeclaration: true,
              FunctionExpression: true,
              MethodDefinition: true
            },
            contexts: [
              "TSInterfaceDeclaration",
              "TSTypeAliasDeclaration",
              "TSEnumDeclaration"
            ]
          }]
        }
      }
    ];
    ESLINT_CONFIG
    NODE_PATH="$modules" eslint --no-config-lookup --config "$config" --format json "$@" |
      jq -c '.[] | .filePath as $file | .messages[]
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
