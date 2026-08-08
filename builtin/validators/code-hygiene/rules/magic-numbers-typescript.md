---
name: magic-numbers-typescript
description: Unnamed TypeScript and JavaScript literals need constants — checked by eslint, not by prompt.
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
supersedes: magic-numbers
tool:
  scope: files
  run: |
    modules="$(cd "$(dirname "$(readlink -f "$(command -v eslint)")")/../.." && pwd -P)"
    config="$(mktemp -d)/eslint.config.cjs"
    cat > "$config" <<'ESLINT_CONFIG'
    const tseslint = require("typescript-eslint");
    module.exports = [
      {
        files: ["**/*.{js,jsx,mjs,cjs,ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
        plugins: { "@typescript-eslint": tseslint.plugin },
        rules: {
          "@typescript-eslint/no-magic-numbers": ["warn", {
            ignore: [0, 1, -1],
            ignoreArrayIndexes: true,
            ignoreDefaultValues: true,
            ignoreClassFieldInitialValues: true,
            ignoreEnums: true,
            ignoreNumericLiteralTypes: true,
            ignoreReadonlyClassProperties: true,
            ignoreTypeIndexes: true
          }]
        }
      }
    ];
    ESLINT_CONFIG
    NODE_PATH="$modules" eslint --no-config-lookup --config "$config" --format json "$@" |
      jq -c '.[] | .filePath as $file | .messages[]
             | select(.ruleId == "@typescript-eslint/no-magic-numbers")
             | {file: $file, line: .line, message: .message}'
  doctor:
    check_command: "which eslint jq"
    check_version_command: "eslint --version"
  install:
    commands:
      - "npm install -g eslint@10.8.0 typescript-eslint@8.66.0 typescript@5.9.3"
---

# Magic Numbers — TypeScript and JavaScript

`eslint` reports every unnamed numeric literal. `no-magic-numbers` names that
check, and the `@typescript-eslint` copy of it is the one this rule runs: the base
rule has no option for an enumeration member, a numeric literal type, a
`readonly` property, or a type index, so on TypeScript it reports all four.

Every option in the config turns off a context the `magic-numbers` prompt rule
carves out. Measured against a probe module holding one literal of each kind, the
base rule reported the array index, the default value, and the enumeration
member; with these options it reported none of them:

- `ignore: [0, 1, -1]` is the prompt carve-out list, spelled as the rule spells it.
- `ignoreArrayIndexes`, `ignoreDefaultValues`, `ignoreClassFieldInitialValues`,
  `ignoreEnums`, `ignoreNumericLiteralTypes`, `ignoreReadonlyClassProperties`,
  and `ignoreTypeIndexes` each name a position where a declaration already names
  the value.
- `enforceConst` and `detectObjects` stay at their defaults, which are off. A
  `const` binding and an object property both name their value.

What is left is the positions where nothing names the number: a comparison, an
operation, and a call argument.

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

`typescript-eslint` supplies both the parser and the plugin. The default eslint
parser cannot read TypeScript syntax, so a `.ts` file without it is a parse error
instead of a finding. The install command pins `typescript` to 5.9.3 because
`typescript-eslint` accepts `typescript` below 6.1.0.

The scope is `files` because eslint reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one literal, write
`// eslint-disable-next-line @typescript-eslint/no-magic-numbers` above it in the
code.
