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
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    modules="$(cd "$(dirname "$(readlink -f "$(command -v eslint)")")/../.." && pwd -P)"
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    config="$work/eslint.config.cjs"
    cat > "$config" <<'ESLINT_CONFIG'
    const tseslint = require("typescript-eslint");
    module.exports = [
      {
        files: ["**/*.{js,jsx,mjs,cjs,ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
        plugins: { "@typescript-eslint": tseslint.plugin },
        rules: {
          "@typescript-eslint/no-magic-numbers": ["warn", {
            ignore: [0, 1, -1, 100],
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

- `ignore: [0, 1, -1, 100]` is the prompt carve-out value list. `0`, `1` and
  `-1` are the first half of it, and `100` is the percent half of "conventional
  values". Measured: with `[0, 1, -1]` both `(part * 100) / total` and
  `usage === 100` reported `No magic number: 100.`; with `100` in the list both
  are silent, and `size * 4096` still reports.
- `ignoreArrayIndexes`, `ignoreDefaultValues`, `ignoreClassFieldInitialValues`,
  `ignoreEnums`, `ignoreNumericLiteralTypes`, `ignoreReadonlyClassProperties`,
  and `ignoreTypeIndexes` each name a position where a declaration already names
  the value.
- `enforceConst` and `detectObjects` stay at their defaults, which are off. A
  `const` binding and an object property both name their value.

What is left is the positions where nothing names the number: a comparison, an
operation, and a call argument.

## The shift carve-out cannot be expressed

The prompt rule names two conventional values, and this rule restores one of
them. `100` for percent is a VALUE, so `ignore` states it. A `<< 8` is a
POSITION — the operand of a shift — and `ignore` selects a value and never a
position.

Measured on the same probe: `word << 8`, `word >> 8` and `word === 8` each
reported `No magic number: 8.`, and `8` added to `ignore` silenced all three. A
list that carried `8` would therefore drop a genuine `status === 8` to keep the
shift silent, which trades a real finding for a carve-out.

No option answers it either. `eslint` validates the option object against the
rule schema and names the whole set the rule accepts — `detectObjects`,
`enforceConst`, `ignore`, `ignoreArrayIndexes`, `ignoreDefaultValues`,
`ignoreClassFieldInitialValues`, `ignoreEnums`, `ignoreNumericLiteralTypes`,
`ignoreReadonlyClassProperties`, and `ignoreTypeIndexes`. None of the ten names
an operand of a shift, and an eleventh key is refused: an added `ignoreShift`
makes `eslint` answer `Unexpected property "ignoreShift"` and stop.

So a shift operand REPORTS, and the recourse is the inline suppression at the
end of this file: write `// eslint-disable-next-line
@typescript-eslint/no-magic-numbers` above the shift, with the reason after it.
The fail fixture carries `word << 8` for that reason, and the acceptance test
`the_shipped_typescript_magic_numbers_tool_rule_reports_every_fail_fixture_value`
holds `eslint` to reporting it, so the gap stays measured.

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

`typescript-eslint` supplies both the parser and the plugin. The default eslint
parser cannot read TypeScript syntax, so a `.ts` file without it is a parse error
instead of a finding. The install command pins `typescript` to 5.9.3 because
`typescript-eslint` accepts `typescript` below 6.1.0.

The scope is `files` because eslint reads the files it is given.

Selection in the pipe is attribution, not exemption: to exempt one literal, write
`// eslint-disable-next-line @typescript-eslint/no-magic-numbers` above it in the
code.

## The run answers for its own arguments

The configuration this rule writes matches
`**/*.{js,jsx,mjs,cjs,ts,tsx}`, and eslint reads the working directory
when it takes no path. A run with no file therefore names an unnamed
literal in each such file under the workspace root, at exit 0. The script
counts its arguments first, and a count of zero exits 0 with no finding.

Measured over two TypeScript files, each comparing against one unnamed
literal and returning another:

| what the script is given | findings |
|---|---|
| no argument, before the guard | 4 |
| no argument, after the guard | 0 |
| the two files | 4 |

The acceptance test in `shipped/magic_numbers.rs` holds the first two
rows.

## The temporary directory the configuration stands in

`mktemp -d` makes the directory the eslint configuration is written into,
and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers a clean run,
a run with findings and a broken run alike. Measured over one file: one
run raised the count of entries under `TMPDIR` by 1 before the trap, and
leaves that count unchanged after it.
