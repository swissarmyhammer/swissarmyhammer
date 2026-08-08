---
name: complexity-swift
description: Swift functions stay under the complexity and length gates — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes:
  - cognitive-complexity
  - function-length
tool:
  scope: files
  run: |
    config="$(mktemp -d)/swiftlint.yml"
    printf '%s\n' 'only_rules:' '  - cyclomatic_complexity' '  - function_body_length' \
      'cyclomatic_complexity:' '  warning: 15' '  ignores_case_statements: true' \
      'function_body_length:' '  warning: 250' > "$config"
    swiftlint lint --config "$config" --no-cache --quiet --reporter json "$@" |
      jq -c '.[] | select(.rule_id == "cyclomatic_complexity"
                          or .rule_id == "function_body_length")
             | {file: .file, line: .line, message: .reason}'
  doctor:
    check_command: "which swiftlint jq"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Complexity and Length — Swift

`swiftlint` decides both gates in one run. Two rules carry it:

- `cyclomatic_complexity` — a function with too many decision points.
- `function_body_length` — a function that runs too long.

One run answers two prompt rules, so this rule names both in `supersedes`.

Swiftlint's whole metrics group is `cyclomatic_complexity`,
`function_body_length`, `closure_body_length`, `nesting`, `file_length`,
`type_body_length` and `line_length`. It has no cognitive-complexity rule, so
the two named above are the pair that answers the two prompt gates.

## What the complexity gate measures, and what goes with it

`cyclomatic_complexity` counts decision points, with no `+1` base: a probe of
one `for`, one `if` and 14 `else if` scores 16, and the rule reports only when
the count is strictly over the warning level. That is not the published Sonar
cognitive complexity the `complexity` probe computes, so the two numbers need
not agree on the same function.

The threshold stays at 15, the number the `cognitive-complexity` prompt rule
states. The prompt rule's second gate — condition-nesting depth 4 or more — has
its own swiftlint rule, `nesting`, but that rule measures type and function
declarations nested inside each other rather than conditions, so it does not
answer the gate. Superseding therefore drops the nesting gate for Swift. That is
the trade the tool rule makes.

## Why `ignores_case_statements` is on

Without the option, every `case` arm of a `switch` adds one. The prompt rule
says the opposite in as many words: "A `match` or `switch` counts once for the
whole construct. Its arms are branches of one decision", and it carves out "a
long flat list of simple cases".

The difference was measured over 893 `.swift` files — Alamofire, swift-nio and
vapor at HEAD, none of which carries a `.swiftlint.yml` of its own, so the
numbers are not the residue of prior linting:

| `warning` | plain | `ignores_case_statements: true` |
|---|---|---|
| 8 | 118 | 21 |
| 10 | 66 | 12 |
| 12 | 44 | 9 |
| 15 | 23 | 2 |
| 20 | 8 | 1 |

At the gate of 15 the plain form reports 23 findings and 21 of them disappear
when case arms stop counting, because they are flat dispatch tables:
`NIOHTTP1/HTTPEncoder.swift` `write(response:)` scores 121 from 120 one-line
`case` arms each writing a status line, and `NIOPosix/HappyEyeballs.swift`
`processInput` scores 26 from a state-machine `switch`. Under this set's
contract a tool finding is a requirement, so the plain form would make 21
suppressions mandatory on code the prompt rule calls correct.

The option is not free. It drops a `switch` to zero rather than to one, so a
function whose branching lives entirely in nested `switch` statements — the
`while` around a `switch` around a `switch` in `_NIOFileSystem/DirectoryEntries.swift`
— passes the gate. The rule takes that under-count: a missed finding leaves the
review where it was, and a wrong finding is a requirement to change correct code.

## The length gate counts what the prompt rule counts

`function_body_length` reports "excluding comments and whitespace", which is the
`function-length` prompt rule's definition word for word. Measured on a probe of
260 code lines carrying 52 comment-only lines and 52 blank lines, swiftlint
reports 262 — the code lines exactly, because the count covers the body and not
the signature line.

Naming only `warning:` also disables the rule's default `error:` level. That
matters for `function_body_length`, whose default error level is 100: the 262-line
probe reports as a warning against 250, not as an error against 100, so the gate
is the number this rule writes and nothing else.

## How the run is shaped

The script writes its own `swiftlint.yml` to a temporary path and passes it with
`--config`. `only_rules` turns the two rules on and every other swiftlint rule
off, so the rule owns its whole invocation and never reads the project's own
`.swiftlint.yml`. `--no-cache` keeps swiftlint from writing a cache directory
into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

The rule declares no install commands. Homebrew is the supported way to install
swiftlint, and it installs the current version only, so a Homebrew command
cannot pin one. Mint can pin one — `mint install realm/SwiftLint@0.65.0` — but
it builds swiftlint from source and links the result into `~/.mint/bin`, which
is not on the path, so the command cannot make `check_command` pass. The
`doctor.fix_hint` states `brew install swiftlint` instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption: to exempt one function,
write `// swiftlint:disable:next cyclomatic_complexity` — or the matching rule
name — above it in the code.
