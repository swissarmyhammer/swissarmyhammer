---
name: magic-numbers-swift
description: Unnamed Swift literals need constants — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: magic-numbers
tool:
  scope: files
  run: |
    config="$(mktemp -d)/swiftlint.yml"
    printf '%s\n' 'only_rules:' '  - no_magic_numbers' \
      'no_magic_numbers:' '  allowed_numbers: [0, 1, -1, 100]' > "$config"
    swiftlint lint --config "$config" --no-cache --quiet --reporter json "$@" |
      jq -c '.[] | select(.rule_id == "no_magic_numbers")
             | {file: .file, line: .line, message: .reason}'
  doctor:
    check_command: "which swiftlint jq"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Magic Numbers — Swift

`swiftlint` reports every unnamed numeric literal. The `no_magic_numbers` rule
names that check. It is opt-in, so it never runs unless a configuration turns it
on.

The rule is already close to the `magic-numbers` prompt carve-outs. Measured
against a probe file holding one literal of each kind, it reported nothing for a
variable declaration, a stored property, a `static let`, an enumeration raw value,
or a default parameter — each of those declarations names its value. `100` is
absent too, which is the prompt carve-out for percent.

`allowed_numbers` is the one threshold the rule sets. The swiftlint default is
`[0.0, 1.0, 100.0]`, which is the prompt carve-out list without `-1`, so the
config states `[0, 1, -1, 100]` and the two lists then agree. `magic-numbers-go`
and `magic-numbers-typescript` state the same four values in their own
allow-lists.

## The shift carve-out is expressed, and Swift alone expresses it

The prompt rule names two conventional values, and this rule restores both.
`100` for percent is a VALUE, so `allowed_numbers` states it. A `<< 8` is a
POSITION — the operand of a shift — and no value allow-list can state a
position. `magic-numbers-go` and `magic-numbers-typescript` each record that
their own list cannot: a list carrying `8` silences a genuine `status == 8`
beside `word << 8`, which trades a real finding for a carve-out.

swiftlint answers it without the list, because `no_magic_numbers` reads the
OPERATOR. Measured on swiftlint 0.65.0 against the shipped
`allowed_numbers: [0, 1, -1, 100]`: `return word << 8` and `return word >> 8`
report nothing, and `return status == 8` reports
`Magic numbers should be replaced by named constants`. Both operands are carved
out, so `return 4096 << width` is silent as well.

The carve-out is the shift operator and nothing else. Measured in one identical
shape, `return word <operator> 8`: `<<` and `>>` are silent, and `&<<` — the
masking shift — `*`, `+`, `&`, `|`, `^` and `==` each report their `8`.

So `8` stays OUT of `allowed_numbers`. The carve-out is already reached, and
adding `8` would buy nothing and lose `status == 8`.

### The carve-out reaches a whole shift, not a link of a longer chain

The carve-out holds when the shift is the WHOLE expression at its position. It
does not reach a shift that stands as one link of a longer unparenthesised
operator chain, because swiftlint then reads the chain rather than a shift.
Measured on the same probe against the same config:

| Written | Reported |
|---|---|
| `return word << 8` | no |
| `let packed = word << 8` | no |
| `schedule(value: word << 8)` | no |
| `acc = (word << 8)` | no |
| `return (word << 8) \| 1` | no |
| `if (word << 8) > 0` | no |
| `acc = word << 8` | yes |
| `return word << 8 \| 1` | yes |
| `if word << 8 > 0` | yes |
| `return flag ? word << 8 : word` | yes |

Two recourses answer the four that report, and both are measured. Parentheses
around the shift silence every one of them, and that is the clearer code
besides. The other is the inline suppression at the end of this file: write
`// swiftlint:disable:next no_magic_numbers` above the line, with the reason
after it, which silenced `return word << 8 | 1`.

No option answers the rest. `swiftlint rules no_magic_numbers` names the whole
set the rule accepts — `severity`, `test_parent_classes` and `allowed_numbers`.
None of the three names a shift, and a fourth key is refused: an added
`allowed_shifts` makes swiftlint answer `Configuration for 'no_magic_numbers'
rule contains the invalid key(s) 'allowed_shifts'.` and read it no further.

Both halves are held by fixtures. The pass fixture carries `return word << 8`
and `return word >> 8`, so a swiftlint release that dropped the carve-out makes
the fixture pair fail and the doctor mark the rule unusable. The fail fixture
carries `return word << 8 | 1`, and the acceptance test
`the_shipped_swift_magic_numbers_tool_rule_reports_every_fail_fixture_line`
holds swiftlint to reporting it, so the edge stays measured.

## How the run is shaped

The script writes its own `swiftlint.yml` to a temporary path and passes it with
`--config`. `only_rules` turns the `no_magic_numbers` rule on and every other
swiftlint rule off, so the rule owns its whole invocation and never reads the
project's own `.swiftlint.yml`. `--no-cache` keeps swiftlint from writing a cache
directory into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

The rule declares no install commands. Homebrew is the supported way to install
swiftlint, and it installs the current version only, so a Homebrew command
cannot pin one. Mint can pin one — `mint install realm/SwiftLint@0.65.0` — but
it builds swiftlint from source and links the result into `~/.mint/bin`, which
is not on the path, so the command cannot make `check_command` pass. The
`doctor.fix_hint` states `brew install swiftlint` instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption: to exempt one literal, write
`// swiftlint:disable:next no_magic_numbers` above it in the code.
